//! Thin wrapper over the demoparser2 core: header, roster and round events.

use crate::error::ParseError;
use crate::extract::{
    build_rounds, derive_tick_rate, fill_events, fill_grenades, fill_snapshots, snapshot_ticks, Dict, MatchEvent, ProjKind,
    ProjPoint, RoundEvent, TickRow,
};
use crate::models::{
    KILL_ATTACKER_BLIND, KILL_FLASH_ASSIST, KILL_HEADSHOT, KILL_NO_SCOPE, KILL_THROUGH_SMOKE, KILL_WALLBANG,
};
use crate::models::{Match, Player, Team, SCHEMA_VERSION};
use ahash::AHashMap;
use memmap2::MmapOptions;
use parser::first_pass::parser_settings::ParserInputs;
use parser::parse_demo::{Parser, ParsingMode};
use parser::first_pass::prop_controller::{
    GAME_TIME_ID, INVENTORY_ID, IS_ALIVE_ID, PITCH_ID, PLAYER_X_ID, PLAYER_Y_ID, PLAYER_Z_ID, STEAMID_ID, TICK_ID,
    WEAPON_NAME_ID, YAW_ID,
};
use parser::second_pass::parser_settings::create_huffman_lookup_table;
use parser::second_pass::variants::{PropColumn, VarVec};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

const DEFAULT_TICK_RATE: u32 = 64;

/// Reads only the demo header (map name etc.).
pub fn read_header(path: &Path) -> Result<BTreeMap<String, String>, ParseError> {
    let file = File::open(path)?;
    // Safety: the file is treated as read-only and must not be modified while parsing.
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let huf = create_huffman_lookup_table();
    let inputs = ParserInputs {
        real_name_to_og_name: AHashMap::default(),
        wanted_players: vec![],
        wanted_player_props: vec![],
        wanted_other_props: vec![],
        wanted_prop_states: AHashMap::default(),
        wanted_ticks: vec![],
        wanted_events: vec![],
        parse_ents: false,
        parse_projectiles: false,
        parse_grenades: false,
        parse_weapons: false,
        only_header: true,
        only_convars: false,
        huffman_lookup_table: &huf,
        order_by_steamid: false,
        list_props: false,
        fallback_bytes: None,
    };
    let mut parser = Parser::new(inputs, ParsingMode::ForceSingleThreaded);
    let out = parser
        .parse_demo(&mmap)
        .map_err(|e| ParseError::Demo(format!("{e:?}")))?;
    Ok(out.header.unwrap_or_default().into_iter().collect())
}

const WANTED_EVENTS: [&str; 18] = [
    "round_start", "round_freeze_end", "round_end", "begin_new_match", "player_death", "player_hurt", "player_blind",
    "bomb_planted", "bomb_defused", "bomb_exploded", "bomb_dropped", "bomb_pickup", "hegrenade_detonate",
    "flashbang_detonate", "smokegrenade_detonate", "smokegrenade_expired", "inferno_startburn", "inferno_expire",
];

type Ev = parser::second_pass::game_events::GameEvent;

fn field<'a>(e: &'a Ev, name: &str) -> Option<&'a parser::second_pass::variants::Variant> {
    e.fields.iter().find(|f| f.name == name)?.data.as_ref()
}

fn flag_field(e: &Ev, name: &str) -> bool {
    matches!(field(e, name), Some(parser::second_pass::variants::Variant::Bool(true)))
}

/// Steam ids in events are decimal strings; 0 and missing mean "nobody" (world, bot, no assister).
fn steam_field(e: &Ev, name: &str) -> Option<u64> {
    use parser::second_pass::variants::Variant;
    match field(e, name)? {
        Variant::String(s) => s.parse().ok().filter(|&v| v != 0),
        Variant::U64(v) => Some(*v).filter(|&v| v != 0),
        _ => None,
    }
}

fn str_field(e: &Ev, name: &str) -> String {
    match field(e, name) {
        Some(parser::second_pass::variants::Variant::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn pos_field(e: &Ev) -> Option<[f32; 3]> {
    use parser::second_pass::variants::Variant;
    let f = |n: &str| match field(e, n) {
        Some(Variant::F32(v)) => Some(*v),
        _ => None,
    };
    Some([f("x")?, f("y")?, f("z")?])
}

fn match_event(e: &Ev) -> Option<MatchEvent> {
    let tick = e.tick;
    let user = steam_field(e, "user_steamid");
    Some(match e.name.as_str() {
        "player_death" => {
            let mut flags = 0;
            for (name, bit) in [
                ("headshot", KILL_HEADSHOT),
                ("thrusmoke", KILL_THROUGH_SMOKE),
                ("noscope", KILL_NO_SCOPE),
                ("attackerblind", KILL_ATTACKER_BLIND),
                ("assistedflash", KILL_FLASH_ASSIST),
            ] {
                if flag_field(e, name) {
                    flags |= bit;
                }
            }
            if int_field(e, "penetrated").unwrap_or(0) > 0 {
                flags |= KILL_WALLBANG;
            }
            MatchEvent::Death {
                tick,
                victim: user?,
                killer: steam_field(e, "attacker_steamid"),
                assister: steam_field(e, "assister_steamid"),
                weapon: str_field(e, "weapon"),
                flags,
            }
        }
        "player_hurt" => MatchEvent::Hurt {
            tick,
            victim: user?,
            attacker: steam_field(e, "attacker_steamid"),
            weapon: str_field(e, "weapon"),
            dmg: int_field(e, "dmg_health").unwrap_or(0).max(0) as u32,
            armor: int_field(e, "dmg_armor").unwrap_or(0).max(0) as u32,
            hp_after: int_field(e, "health").unwrap_or(0).max(0) as u32,
        },
        "player_blind" => {
            let secs = match field(e, "blind_duration") {
                Some(parser::second_pass::variants::Variant::F32(v)) => *v,
                _ => 0.0,
            };
            MatchEvent::Blind { tick, victim: user?, thrower: steam_field(e, "attacker_steamid"), ms: (secs * 1000.0).round() as u32 }
        }
        "bomb_planted" => MatchEvent::BombPlanted { tick, player: user? },
        "bomb_defused" => MatchEvent::BombDefused { tick, player: user? },
        "bomb_exploded" => MatchEvent::BombExploded { tick, player: user.unwrap_or(0) },
        "bomb_dropped" => MatchEvent::BombDropped { tick, player: user? },
        "bomb_pickup" => MatchEvent::BombPickup { tick, player: user? },
        "hegrenade_detonate" | "flashbang_detonate" | "smokegrenade_detonate" => MatchEvent::Detonate {
            kind: match e.name.as_str() {
                "hegrenade_detonate" => ProjKind::He,
                "flashbang_detonate" => ProjKind::Flash,
                _ => ProjKind::Smoke,
            },
            tick,
            entity: int_field(e, "entityid")?,
            thrower: user?,
            pos: pos_field(e)?,
        },
        "smokegrenade_expired" => MatchEvent::SmokeExpired { tick, entity: int_field(e, "entityid")? },
        "inferno_startburn" => MatchEvent::FireStart { tick, entity: int_field(e, "entityid")?, thrower: user?, pos: pos_field(e)? },
        "inferno_expire" => MatchEvent::FireEnd { tick, entity: int_field(e, "entityid")? },
        _ => return None,
    })
}

fn int_field(e: &parser::second_pass::game_events::GameEvent, name: &str) -> Option<i32> {
    use parser::second_pass::variants::Variant;
    match e.fields.iter().find(|f| f.name == name)?.data.as_ref()? {
        Variant::I32(v) => Some(*v),
        Variant::U32(v) => Some(*v as i32),
        _ => None,
    }
}

fn round_event(e: &parser::second_pass::game_events::GameEvent) -> Option<RoundEvent> {
    let tick = e.tick;
    match e.name.as_str() {
        "begin_new_match" => Some(RoundEvent::NewMatch(tick)),
        "round_start" => Some(RoundEvent::Start(tick)),
        "round_freeze_end" => Some(RoundEvent::FreezeEnd(tick)),
        "round_end" => Some(RoundEvent::End {
            tick,
            winner: int_field(e, "winner").unwrap_or(0),
            reason: int_field(e, "reason").unwrap_or(0),
        }),
        _ => None,
    }
}

/// Cheap pass: map, player roster and rounds (start/end/winner), without snapshots.
pub fn read_match_info(path: &Path) -> Result<Match, ParseError> {
    read_events_pass(path).map(|(m, _)| m)
}

/// Pass 1: header, roster, rounds and the replay events.
fn read_events_pass(path: &Path) -> Result<(Match, Vec<MatchEvent>), ParseError> {
    let file = File::open(path)?;
    // Safety: read-only mapping, the file must not change while parsing.
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let huf = create_huffman_lookup_table();
    let inputs = ParserInputs {
        real_name_to_og_name: AHashMap::default(),
        wanted_players: vec![],
        wanted_player_props: vec![],
        wanted_other_props: vec![],
        wanted_prop_states: AHashMap::default(),
        wanted_ticks: vec![],
        wanted_events: WANTED_EVENTS.iter().map(|s| s.to_string()).collect(),
        parse_ents: true,
        parse_projectiles: false,
        parse_grenades: false,
        parse_weapons: false,
        only_header: false,
        only_convars: false,
        huffman_lookup_table: &huf,
        order_by_steamid: false,
        list_props: false,
        fallback_bytes: None,
    };
    let out = Parser::new(inputs, ParsingMode::ForceSingleThreaded)
        .parse_demo(&mmap)
        .map_err(|e| ParseError::Demo(format!("{e:?}")))?;

    // End-of-match scoreboard first; the live roster covers demos without it.
    let source = if out.player_md.is_empty() { &out.roster } else { &out.player_md };
    let mut raw: Vec<_> = source
        .iter()
        .filter_map(|p| Some((p.steamid?, p.name.clone().unwrap_or_default(), p.team_number)))
        .collect();
    // Stable order independent of hash-map iteration.
    raw.sort_by_key(|(steam_id, _, _)| *steam_id);
    raw.dedup_by_key(|(steam_id, _, _)| *steam_id);
    let players = raw
        .into_iter()
        .enumerate()
        .map(|(i, (steam_id, name, team))| Player {
            index: i as u32,
            steam_id,
            name,
            final_team: team.and_then(Team::from_team_num),
        })
        .collect();

    let round_events: Vec<RoundEvent> = out.game_events.iter().filter_map(round_event).collect();

    let events: Vec<MatchEvent> = out.game_events.iter().filter_map(match_event).collect();

    let m = Match {
        schema_version: SCHEMA_VERSION,
        map: out
            .header
            .as_ref()
            .and_then(|h| h.get("map_name").cloned())
            .unwrap_or_default(),
        // The demo does not expose it; `parse` derives it from game time in the tick pass.
        tick_rate: DEFAULT_TICK_RATE,
        players,
        weapons: vec![],
        rounds: build_rounds(&round_events),
    };
    Ok((m, events))
}

/// Full parse: rounds and events plus per-round snapshots (every 4 ticks), economy, bomb and the
/// real tick rate, and grenades. The demo is read three times (events, player state, projectiles):
/// the demoparser2 core cannot collect events and entity state in one pass.
pub fn parse(path: &Path) -> Result<Match, ParseError> {
    let (mut m, events) = read_events_pass(path)?;
    if m.rounds.is_empty() {
        return Ok(m);
    }
    let rows = read_tick_rows(path, snapshot_ticks(&m.rounds))?;
    m.tick_rate = derive_tick_rate(&rows, &m.rounds);
    let mut dict = Dict::default();
    fill_snapshots(&mut m.rounds, &m.players, &rows, &mut dict);
    fill_events(&mut m.rounds, &m.players, &events, &mut dict);
    let points = read_projectiles(path)?;
    fill_grenades(&mut m.rounds, &m.players, &points, &events);
    m.weapons = dict.names;
    Ok(m)
}

/// Friendly property names the tick pass asks for. The core wants the original
/// (`CCSPlayerPawn.m_iHealth`-style) names for everything except its built-in columns.
const TICK_PROPS: [&str; 11] = [
    "X", "Y", "Z", "pitch", "yaw", "health", "team_num", "balance", "armor_value", "has_helmet", "has_defuser",
];

fn read_tick_rows(path: &Path, ticks: Vec<i32>) -> Result<Vec<TickRow>, ParseError> {
    use parser::maps::FRIENDLY_NAMES_MAPPING as FRIENDLY;
    let file = File::open(path)?;
    // Safety: read-only mapping, the file must not change while parsing.
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let huf = create_huffman_lookup_table();
    let og = |name: &str| FRIENDLY.get(name).map_or(name.to_string(), |o| o.to_string());
    let mut props: Vec<String> = TICK_PROPS.iter().map(|n| og(n)).collect();
    props.push(og("cash_spent_this_round"));
    props.extend(["inventory", "is_alive", "active_weapon_name"].map(og));
    let inputs = ParserInputs {
        real_name_to_og_name: AHashMap::default(),
        wanted_players: vec![],
        wanted_player_props: props,
        wanted_other_props: vec!["game_time".into()],
        wanted_prop_states: AHashMap::default(),
        wanted_ticks: ticks,
        wanted_events: vec![],
        parse_ents: true,
        parse_projectiles: false,
        parse_grenades: false,
        parse_weapons: false,
        only_header: false,
        only_convars: false,
        huffman_lookup_table: &huf,
        order_by_steamid: false,
        list_props: false,
        fallback_bytes: None,
    };
    let out = Parser::new(inputs, ParsingMode::ForceSingleThreaded)
        .parse_demo(&mmap)
        .map_err(|e| ParseError::Demo(format!("{e:?}")))?;

    let named = |friendly: &str| out.prop_controller.name_to_id.get(&og(friendly)).and_then(|id| out.df.get(id));
    let col = |id: u32| out.df.get(&id);
    let (tick, steam) = (col(TICK_ID), col(STEAMID_ID));
    let (Some(tick), Some(steam)) = (tick, steam) else { return Ok(vec![]) };
    let (x, y, z) = (col(PLAYER_X_ID), col(PLAYER_Y_ID), col(PLAYER_Z_ID));
    let (yaw, pitch) = (col(YAW_ID), col(PITCH_ID));
    let (alive, weapon, inventory, game_time) = (col(IS_ALIVE_ID), col(WEAPON_NAME_ID), col(INVENTORY_ID), col(GAME_TIME_ID));
    let (hp, team, money, armor) = (named("health"), named("team_num"), named("balance"), named("armor_value"));
    let (helmet, defuser, spent) = (named("has_helmet"), named("has_defuser"), named("cash_spent_this_round"));

    let n = tick.len();
    let mut rows = Vec::with_capacity(n);
    for i in 0..n {
        let (Some(t), Some(sid)) = (num(Some(tick), i), steam_id(steam, i)) else { continue };
        rows.push(TickRow {
            tick: t as i32,
            steam_id: sid,
            x: num(x, i).map(|v| v as f32),
            y: num(y, i).map(|v| v as f32),
            z: num(z, i).map(|v| v as f32),
            yaw: num(yaw, i).map(|v| v as f32),
            pitch: num(pitch, i).map(|v| v as f32),
            hp: num(hp, i).map(|v| v as i32),
            alive: flag(alive, i),
            team: num(team, i).map(|v| v as i32),
            weapon: text(weapon, i),
            money: num(money, i).map(|v| v as i32),
            armor: num(armor, i).map(|v| v as i32),
            helmet: flag(helmet, i),
            defuser: flag(defuser, i),
            inventory: texts(inventory, i),
            spent: num(spent, i).map(|v| v as i32),
            game_time: num(game_time, i).map(|v| v as f32),
        });
    }
    Ok(rows)
}

// Typed readers over the core's column storage.
fn num(c: Option<&PropColumn>, i: usize) -> Option<f64> {
    match c?.data.as_ref()? {
        VarVec::F32(v) => v.get(i).copied().flatten().map(f64::from),
        VarVec::I32(v) => v.get(i).copied().flatten().map(f64::from),
        VarVec::U32(v) => v.get(i).copied().flatten().map(f64::from),
        _ => None,
    }
}

// Steam ids exceed f64's exact integer range, so they are read as u64.
fn steam_id(c: &PropColumn, i: usize) -> Option<u64> {
    match c.data.as_ref()? {
        VarVec::U64(v) => v.get(i).copied().flatten(),
        _ => None,
    }
}

fn flag(c: Option<&PropColumn>, i: usize) -> Option<bool> {
    match c?.data.as_ref()? {
        VarVec::Bool(v) => v.get(i).copied().flatten(),
        _ => None,
    }
}

fn text(c: Option<&PropColumn>, i: usize) -> Option<String> {
    match c?.data.as_ref()? {
        VarVec::String(v) => v.get(i).cloned().flatten(),
        _ => None,
    }
}

fn texts(c: Option<&PropColumn>, i: usize) -> Vec<String> {
    match c.and_then(|c| c.data.as_ref()) {
        Some(VarVec::StringVec(v)) => v.get(i).cloned().unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Pass 3: every grenade projectile on every tick (all ticks; filtered to rounds later).
fn read_projectiles(path: &Path) -> Result<Vec<ProjPoint>, ParseError> {
    use parser::first_pass::prop_controller::{ENTITY_ID_ID, GRENADE_TYPE_ID, GRENADE_X, GRENADE_Y, GRENADE_Z};
    let file = File::open(path)?;
    // Safety: read-only mapping, the file must not change while parsing.
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let huf = create_huffman_lookup_table();
    let inputs = ParserInputs {
        real_name_to_og_name: AHashMap::default(),
        wanted_players: vec![],
        wanted_player_props: vec![],
        wanted_other_props: vec![],
        wanted_prop_states: AHashMap::default(),
        wanted_ticks: vec![],
        wanted_events: vec![],
        parse_ents: true,
        parse_projectiles: true,
        parse_grenades: false,
        parse_weapons: false,
        only_header: false,
        only_convars: false,
        huffman_lookup_table: &huf,
        order_by_steamid: false,
        list_props: false,
        fallback_bytes: None,
    };
    let out = Parser::new(inputs, ParsingMode::ForceSingleThreaded)
        .parse_demo(&mmap)
        .map_err(|e| ParseError::Demo(format!("{e:?}")))?;
    // Note: `out.projectiles` stays empty in the core; the data is in the dataframe columns.
    let col = |id: u32| out.df.get(&id).and_then(|c| c.data.as_ref());
    let (Some(VarVec::String(ty)), Some(VarVec::I32(tick)), Some(VarVec::I32(ent)), Some(VarVec::U64(sid))) =
        (col(GRENADE_TYPE_ID), col(TICK_ID), col(ENTITY_ID_ID), col(STEAMID_ID))
    else {
        return Ok(vec![]);
    };
    let coord = |id: u32| match col(id) {
        Some(VarVec::F32(v)) => v.as_slice(),
        _ => &[],
    };
    let (xs, ys, zs) = (coord(GRENADE_X), coord(GRENADE_Y), coord(GRENADE_Z));
    let mut points = Vec::with_capacity(ty.len());
    for i in 0..ty.len() {
        let kind = match ty[i].as_deref() {
            Some("CSmokeGrenadeProjectile") => ProjKind::Smoke,
            Some("CHEGrenadeProjectile") => ProjKind::He,
            Some("CFlashbangProjectile") => ProjKind::Flash,
            Some("CMolotovProjectile") | Some("CIncendiaryGrenadeProjectile") => ProjKind::Fire,
            Some("CDecoyProjectile") => ProjKind::Decoy,
            _ => continue,
        };
        let get = |v: &[Option<f32>]| v.get(i).copied().flatten();
        let (Some(t), Some(e), Some(s), Some(x), Some(y), Some(z)) =
            (tick[i], ent[i], sid[i], get(xs), get(ys), get(zs))
        else {
            continue;
        };
        points.push(ProjPoint { tick: t, entity: e, steam_id: s, kind, pos: [x, y, z] });
    }
    Ok(points)
}

//! Raw demo data -> model structures. Pure functions, testable without a demo file.

use crate::models::*;
use std::collections::HashMap;

/// Snapshot step in ticks.
pub const SNAPSHOT_STEP: i32 = 4;
const DEFAULT_TICK_RATE: u32 = 64;

/// One player at one snapshot tick, as read from the demo. `None` means the game had no
/// value (e.g. a dead player has no pawn).
#[derive(Debug, Clone, Default)]
pub struct TickRow {
    pub tick: i32,
    pub steam_id: u64,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub z: Option<f32>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub hp: Option<i32>,
    pub alive: Option<bool>,
    pub team: Option<i32>,
    pub weapon: Option<String>,
    pub money: Option<i32>,
    pub armor: Option<i32>,
    pub helmet: Option<bool>,
    pub defuser: Option<bool>,
    pub inventory: Vec<String>,
    pub spent: Option<i32>,
    pub game_time: Option<f32>,
}

/// Round-related game events in the order the demo emitted them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundEvent {
    /// `begin_new_match`: everything before it (warmup, restarts) is discarded.
    NewMatch(i32),
    Start(i32),
    FreezeEnd(i32),
    End { tick: i32, winner: i32, reason: i32 },
}

/// Splits the match into rounds. A round exists only if it has both a start and an end;
/// a trailing unfinished round (demo cut off) is dropped.
pub fn build_rounds(events: &[RoundEvent]) -> Vec<Round> {
    let mut rounds: Vec<Round> = Vec::new();
    let mut start: Option<i32> = None;
    let mut freeze: Option<i32> = None;
    for ev in events {
        match *ev {
            RoundEvent::NewMatch(t) => {
                rounds.clear();
                start = Some(t);
                freeze = None;
            }
            RoundEvent::Start(t) => {
                start = Some(t);
                freeze = None;
            }
            RoundEvent::FreezeEnd(t) => freeze = Some(t),
            RoundEvent::End { tick, winner, reason } => {
                if let Some(start_tick) = start.take() {
                    rounds.push(Round {
                        number: rounds.len() as u32 + 1,
                        start_tick,
                        freeze_end_tick: freeze.take(),
                        end_tick: tick,
                        winner: Team::from_team_num(winner),
                        ticks: Vec::new(),
                        teams: Vec::new(),
                        tracks: Vec::new(),
                        events: RoundEvents::default(),
                        bomb: Bomb::default(),
                        grenades: Vec::new(),
                        reason,
                    });
                }
            }
        }
    }
    rounds
}

/// Ticks to sample: every `SNAPSHOT_STEP` from each round's start to its end.
pub fn snapshot_ticks(rounds: &[Round]) -> Vec<i32> {
    rounds
        .iter()
        .flat_map(|r| (r.start_tick..=r.end_tick).step_by(SNAPSHOT_STEP as usize))
        .collect()
}

/// Tick rate from `game_time` over the first round (ticks / seconds), rounded.
pub fn derive_tick_rate(rows: &[TickRow], rounds: &[Round]) -> u32 {
    let limit = rounds.first().map_or(i32::MAX, |r| r.end_tick);
    let mut timed = rows.iter().filter(|r| r.tick <= limit).filter_map(|r| Some((r.tick, r.game_time?)));
    let Some(first) = timed.next() else { return DEFAULT_TICK_RATE };
    let Some(last) = timed.last() else { return DEFAULT_TICK_RATE };
    let (dt, dg) = ((last.0 - first.0) as f32, last.1 - first.1);
    if dt <= 0.0 || dg <= 0.0 {
        return DEFAULT_TICK_RATE;
    }
    let rate = (dt / dg).round() as u32;
    if (16..=256).contains(&rate) { rate } else { DEFAULT_TICK_RATE }
}

fn tenths(v: f32) -> i32 {
    (v * 10.0).round() as i32
}

/// Interns names into the weapon dictionary (id = position).
#[derive(Default)]
pub struct Dict {
    pub names: Vec<String>,
    ids: HashMap<String, u32>,
}

impl Dict {
    pub fn get(&self, name: &str) -> Option<u32> {
        self.ids.get(name).copied()
    }

    pub fn id(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        self.names.push(name.to_string());
        self.ids.insert(name.to_string(), id);
        id
    }
}

/// Fills `ticks`, `teams` and `tracks` of every round from the sampled rows and extends the
/// weapon dictionary (`dict`). `players` must be indexed by `Player::index`.
pub fn fill_snapshots(rounds: &mut [Round], players: &[Player], rows: &[TickRow], dict: &mut Dict) {
    let by_steam: HashMap<u64, usize> = players.iter().map(|p| (p.steam_id, p.index as usize)).collect();
    let by_key: HashMap<(i32, usize), &TickRow> = rows
        .iter()
        .filter_map(|r| Some(((r.tick, *by_steam.get(&r.steam_id)?), r)))
        .collect();
    for round in rounds.iter_mut() {
        round.ticks = (round.start_tick..=round.end_tick).step_by(SNAPSHOT_STEP as usize).collect();
        round.teams = vec![None; players.len()];
        round.tracks = Vec::with_capacity(players.len());
        // Last snapshot of the buy phase: the end of freeze time, or the round end.
        let buy_idx = round
            .ticks
            .iter()
            .rposition(|&t| round.freeze_end_tick.map_or(true, |f| t <= f))
            .unwrap_or(0);

        for p in 0..players.len() {
            let mut tr = PlayerTrack::default();
            let (mut pos, mut ang) = ([0f32; 3], [0f32; 2]);
            let mut weapon: Option<u32> = None;
            let mut inv: Option<(i32, u8, Vec<u32>)> = None;
            let mut money = 0;
            for (i, &tick) in round.ticks.iter().enumerate() {
                let row = by_key.get(&(tick, p)).copied();
                let alive = row.map_or(false, |r| r.alive.unwrap_or(true) && r.hp.unwrap_or(0) > 0);
                if let Some(r) = row {
                    if let (Some(x), Some(y), Some(z)) = (r.x, r.y, r.z) {
                        pos = [x, y, z];
                    }
                    if let (Some(yaw), Some(pitch)) = (r.yaw, r.pitch) {
                        ang = [yaw, pitch];
                    }
                    if round.teams[p].is_none() {
                        round.teams[p] = r.team.and_then(Team::from_team_num);
                    }
                    money = r.money.unwrap_or(money);
                    if alive {
                        if let Some(w) = &r.weapon {
                            let id = dict.id(w);
                            if weapon != Some(id) {
                                weapon = Some(id);
                                tr.weapon.push((i as u32, id));
                            }
                        }
                        let items: Vec<u32> = r.inventory.iter().map(|n| dict.id(n)).collect();
                        let cur = (
                            r.armor.unwrap_or(0),
                            r.helmet.unwrap_or(false) as u8 | (r.defuser.unwrap_or(false) as u8) << 1,
                            items,
                        );
                        if inv.as_ref() != Some(&cur) {
                            tr.inventory.push(InventoryChange(i as u32, cur.0, cur.1, cur.2.clone()));
                            inv = Some(cur);
                        }
                    }
                    if i == buy_idx {
                        tr.spent = r.spent.unwrap_or(0);
                    }
                }
                tr.x.push(tenths(pos[0]));
                tr.y.push(tenths(pos[1]));
                tr.z.push(tenths(pos[2]));
                tr.yaw.push(tenths(ang[0]));
                tr.pitch.push(tenths(ang[1]));
                tr.hp.push(if alive { row.and_then(|r| r.hp).unwrap_or(0).clamp(1, 100) } else { 0 });
                tr.money.push(money);
            }
            round.tracks.push(tr);
        }
    }
}

/// A game event relevant to the replay; steam ids are raw, mapped to players in `fill_events`.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchEvent {
    Death { tick: i32, victim: u64, killer: Option<u64>, assister: Option<u64>, weapon: String, flags: u8 },
    Hurt { tick: i32, victim: u64, attacker: Option<u64>, weapon: String, dmg: u32, armor: u32, hp_after: u32 },
    Blind { tick: i32, victim: u64, thrower: Option<u64>, ms: u32 },
    BombPlanted { tick: i32, player: u64 },
    BombDefused { tick: i32, player: u64 },
    BombExploded { tick: i32, player: u64 },
    BombDropped { tick: i32, player: u64 },
    BombPickup { tick: i32, player: u64 },
    /// HE / flash / smoke detonation (`entity` is the projectile entity).
    Detonate { kind: ProjKind, tick: i32, entity: i32, thrower: u64, pos: [f32; 3] },
    SmokeExpired { tick: i32, entity: i32 },
    /// Molotov / incendiary fire starts (`entity` is the inferno entity, not the projectile).
    FireStart { tick: i32, entity: i32, thrower: u64, pos: [f32; 3] },
    FireEnd { tick: i32, entity: i32 },
}

impl MatchEvent {
    fn tick(&self) -> i32 {
        match self {
            Self::Death { tick, .. } | Self::Hurt { tick, .. } | Self::Blind { tick, .. } => *tick,
            Self::Detonate { tick, .. }
            | Self::SmokeExpired { tick, .. }
            | Self::FireStart { tick, .. }
            | Self::FireEnd { tick, .. } => *tick,
            Self::BombPlanted { tick, .. }
            | Self::BombDefused { tick, .. }
            | Self::BombExploded { tick, .. }
            | Self::BombDropped { tick, .. }
            | Self::BombPickup { tick, .. } => *tick,
        }
    }
}

fn snapshot_index(r: &Round, tick: i32) -> Option<usize> {
    if r.ticks.is_empty() {
        return None;
    }
    Some((((tick - r.start_tick) / SNAPSHOT_STEP).max(0) as usize).min(r.ticks.len() - 1))
}

fn spot(r: &Round, tick: i32, p: usize) -> Option<(i32, i32, i32)> {
    let (i, t) = (snapshot_index(r, tick)?, r.tracks.get(p)?);
    Some((t.x[i], t.y[i], t.z[i]))
}

/// Distributes events over rounds and builds each round's `events` and `bomb`.
/// Must run after `fill_snapshots` (uses tracks for positions and the bomb carrier).
pub fn fill_events(rounds: &mut [Round], players: &[Player], events: &[MatchEvent], dict: &mut Dict) {
    let by_steam: HashMap<u64, usize> = players.iter().map(|p| (p.steam_id, p.index as usize)).collect();
    let idx = |s: u64| by_steam.get(&s).copied();
    let opt = |s: Option<u64>| s.and_then(idx).map_or(-1, |i| i as i32);
    let starts: Vec<i32> = rounds.iter().map(|r| r.start_tick).collect();

    for ev in events {
        // Round r owns [start_r, start_{r+1}).
        let k = starts.partition_point(|&s| s <= ev.tick());
        if k == 0 {
            continue;
        }
        // Nothing follows the last round except demo teardown (players leaving), so cut it there.
        if k == rounds.len() && ev.tick() > rounds[k - 1].end_tick {
            continue;
        }
        let round = &mut rounds[k - 1];
        match ev {
            MatchEvent::Death { tick, victim, killer, assister, weapon, flags } => {
                if let Some(v) = idx(*victim) {
                    let w = dict.id(weapon);
                    round.events.kills.push(Kill(*tick, opt(*killer), v as u32, opt(*assister), w, *flags));
                }
            }
            MatchEvent::Hurt { tick, victim, attacker, weapon, dmg, armor, hp_after } => {
                if let Some(v) = idx(*victim) {
                    let w = dict.id(weapon);
                    round.events.damage.push(Damage(*tick, opt(*attacker), v as u32, w, *dmg, *armor, *hp_after));
                }
            }
            MatchEvent::Blind { tick, victim, thrower, ms } => {
                if let Some(v) = idx(*victim) {
                    round.events.blinds.push(Blind(*tick, opt(*thrower), v as u32, *ms));
                }
            }
            MatchEvent::BombPlanted { tick, player } => {
                if round.bomb.plant_tick.is_none() {
                    let p = idx(*player);
                    round.bomb.plant_tick = Some(*tick);
                    round.bomb.planter = p.map(|p| p as u32);
                    if let Some((x, y, z)) = p.and_then(|p| spot(round, *tick, p)) {
                        (round.bomb.x, round.bomb.y, round.bomb.z) = (Some(x), Some(y), Some(z));
                    }
                }
            }
            MatchEvent::BombDefused { tick, player } | MatchEvent::BombExploded { tick, player } => {
                round.bomb.end_tick = Some(*tick);
                if matches!(ev, MatchEvent::BombDefused { .. }) {
                    round.bomb.end_type = Some(BombEnd::Defused);
                    round.bomb.defuser = idx(*player).map(|p| p as u32);
                } else {
                    round.bomb.end_type = Some(BombEnd::Exploded);
                }
            }
            MatchEvent::Detonate { .. } | MatchEvent::SmokeExpired { .. } | MatchEvent::FireStart { .. } | MatchEvent::FireEnd { .. } => {}
            MatchEvent::BombDropped { tick, player } | MatchEvent::BombPickup { tick, player } => {
                if let Some(p) = idx(*player) {
                    let (x, y, z) = spot(round, *tick, p).unwrap_or((0, 0, 0));
                    let s = BombSpot(*tick, p as u32, x, y, z);
                    if matches!(ev, MatchEvent::BombDropped { .. }) {
                        round.bomb.drops.push(s);
                    } else {
                        round.bomb.pickups.push(s);
                    }
                }
            }
        }
    }

    let c4 = dict.get("C4 Explosive");
    for round in rounds.iter_mut() {
        build_bomb_track(round, c4);
    }
}

/// Bomb position before the plant: the carrier (alive player holding C4) or the last place it was
/// held. Derived from inventories, so it also covers the bomb being dropped on death.
fn build_bomb_track(round: &mut Round, c4: Option<u32>) {
    let Some(c4) = c4 else { return };
    let n = round.ticks.len();
    // Snapshots up to and including the plant tick.
    let end = round.bomb.plant_tick.map_or(n, |pt| {
        round.ticks.iter().rposition(|&t| t <= pt).map_or(0, |i| i + 1)
    });
    let mut cursor = vec![0usize; round.tracks.len()];
    let mut has_c4 = vec![false; round.tracks.len()];
    let mut last = (0, 0, 0);
    let mut carrier = i32::MIN;
    let b = &mut round.bomb;
    for i in 0..end {
        let mut holder = -1;
        for (p, t) in round.tracks.iter().enumerate() {
            while cursor[p] < t.inventory.len() && t.inventory[cursor[p]].0 as usize <= i {
                has_c4[p] = t.inventory[cursor[p]].3.contains(&c4);
                cursor[p] += 1;
            }
            if has_c4[p] && t.hp[i] > 0 {
                holder = p as i32;
                last = (t.x[i], t.y[i], t.z[i]);
            }
        }
        if holder != carrier {
            b.carrier.push((i as u32, holder));
            carrier = holder;
        }
        b.track_x.push(last.0);
        b.track_y.push(last.1);
        b.track_z.push(last.2);
    }
}

/// Projectile classes the core reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjKind {
    Smoke,
    He,
    Flash,
    /// Molotov and incendiary share a class; the thrower's side tells them apart.
    Fire,
    Decoy,
}

/// One projectile entity at one tick (every tick, positions in game units).
#[derive(Debug, Clone)]
pub struct ProjPoint {
    pub tick: i32,
    pub entity: i32,
    pub steam_id: u64,
    pub kind: ProjKind,
    pub pos: [f32; 3],
}

const GRAVITY: f32 = 800.0;
/// Velocity change per tick beyond free fall that counts as a collision (game units / tick).
const BOUNCE_DELTA: f32 = 1.0;

/// A contiguous stretch of one projectile entity (entity ids are reused over time).
struct Flight<'a> {
    pts: Vec<&'a ProjPoint>,
}

fn split_flights(points: &[ProjPoint]) -> Vec<Flight<'_>> {
    let mut by_entity: HashMap<i32, Vec<&ProjPoint>> = HashMap::new();
    for p in points {
        by_entity.entry(p.entity).or_default().push(p);
    }
    let mut out = Vec::new();
    for (_, mut pts) in by_entity {
        pts.sort_by_key(|p| p.tick);
        let mut cur: Vec<&ProjPoint> = Vec::new();
        for p in pts {
            if let Some(last) = cur.last() {
                if p.tick - last.tick > 1 || p.steam_id != last.steam_id || p.kind != last.kind {
                    out.push(Flight { pts: std::mem::take(&mut cur) });
                }
            }
            cur.push(p);
        }
        if !cur.is_empty() {
            out.push(Flight { pts: cur });
        }
    }
    out.sort_by_key(|f| (f.pts[0].tick, f.pts[0].entity));
    out
}

/// Ticks at which the velocity changed more than free fall explains, merged when adjacent.
fn find_bounces(pts: &[&ProjPoint], tick_rate: u32) -> Vec<usize> {
    let g = GRAVITY / (tick_rate as f32 * tick_rate as f32);
    let vel = |i: usize| -> [f32; 3] {
        let (a, b) = (pts[i - 1].pos, pts[i].pos);
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
    };
    let mut out: Vec<usize> = Vec::new();
    for i in 2..pts.len() {
        let (v0, v1) = (vel(i - 1), vel(i));
        let d = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2] + g];
        if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > BOUNCE_DELTA {
            if out.last().map_or(true, |&l| i - l > 2) {
                out.push(i);
            }
        }
    }
    out
}

fn tenths3(p: [f32; 3]) -> (i32, i32, i32) {
    (tenths(p[0]), tenths(p[1]), tenths(p[2]))
}

/// Builds `grenades` of every round from projectile positions (every tick) and the detonation /
/// fire events. Needs `teams` (set by `fill_snapshots`) to tell molotovs from incendiaries.
pub fn fill_grenades(rounds: &mut [Round], players: &[Player], points: &[ProjPoint], events: &[MatchEvent], tick_rate: u32) {
    let by_steam: HashMap<u64, usize> = players.iter().map(|p| (p.steam_id, p.index as usize)).collect();
    let starts: Vec<i32> = rounds.iter().map(|r| r.start_tick).collect();
    let mut used_fire = vec![false; events.len()];

    for fl in split_flights(points) {
        let (first, last) = (fl.pts[0], *fl.pts.last().unwrap());
        let Some(&thrower) = by_steam.get(&first.steam_id) else { continue };
        let k = starts.partition_point(|&s| s <= first.tick);
        if k == 0 || (k == rounds.len() && first.tick > rounds[k - 1].end_tick) {
            continue;
        }

        // Flight end and what happens next, from the events.
        let mut end_tick = last.tick;
        let mut end_pos = last.pos;
        let mut area: Option<Area> = None;
        let near = |t: i32| t >= first.tick - 1 && t <= last.tick + 2;
        let find = |pred: &dyn Fn(&MatchEvent) -> bool| events.iter().position(|e| pred(e));
        match first.kind {
            ProjKind::He | ProjKind::Flash | ProjKind::Smoke => {
                let hit = find(&|e| matches!(e, MatchEvent::Detonate { kind, entity, tick, .. }
                    if *kind == first.kind && *entity == first.entity && near(*tick)));
                if let Some(MatchEvent::Detonate { tick, pos, .. }) = hit.map(|i| &events[i]) {
                    (end_tick, end_pos) = (*tick, *pos);
                    if first.kind == ProjKind::Smoke {
                        let (x, y, z) = tenths3(*pos);
                        let expired = events.iter().find_map(|e| match e {
                            MatchEvent::SmokeExpired { tick: t, entity } if *entity == first.entity && *t > *tick => Some(*t),
                            _ => None,
                        });
                        area = Some(Area { start_tick: *tick, end_tick: expired, x, y, z });
                    }
                }
            }
            ProjKind::Fire => {
                let hit = (0..events.len()).find(|&i| !used_fire[i] && matches!(&events[i],
                    MatchEvent::FireStart { tick, thrower: t, .. }
                    if *t == first.steam_id && *tick >= last.tick - 2 && *tick <= last.tick + 64));
                if let Some(i) = hit {
                    used_fire[i] = true;
                    if let MatchEvent::FireStart { tick, entity, pos, .. } = &events[i] {
                        (end_tick, end_pos) = (*tick, *pos);
                        let (x, y, z) = tenths3(*pos);
                        let expired = events.iter().find_map(|e| match e {
                            MatchEvent::FireEnd { tick: t, entity: en } if en == entity && *t > *tick => Some(*t),
                            _ => None,
                        });
                        area = Some(Area { start_tick: *tick, end_tick: expired, x, y, z });
                    }
                }
            }
            ProjKind::Decoy => {}
        }

        // Trajectory: run positions up to the end of the flight, every 4 ticks, plus the end point.
        let flight: Vec<&ProjPoint> = fl.pts.iter().copied().filter(|p| p.tick <= end_tick).collect();
        let (mut ticks, mut xs, mut ys, mut zs) = (vec![], vec![], vec![], vec![]);
        for (i, p) in flight.iter().enumerate() {
            if i % SNAPSHOT_STEP as usize == 0 {
                let (x, y, z) = tenths3(p.pos);
                ticks.push(p.tick);
                xs.push(x);
                ys.push(y);
                zs.push(z);
            }
        }
        if ticks.last() != Some(&end_tick) {
            let (x, y, z) = tenths3(end_pos);
            ticks.push(end_tick);
            xs.push(x);
            ys.push(y);
            zs.push(z);
        }
        let bounces = find_bounces(&flight, tick_rate)
            .into_iter()
            .map(|i| {
                let (x, y, z) = tenths3(flight[i - 1].pos);
                Bounce(flight[i - 1].tick, x, y, z)
            })
            .collect();

        let round = &mut rounds[k - 1];
        let kind = match first.kind {
            ProjKind::Smoke => GrenadeKind::Smoke,
            ProjKind::He => GrenadeKind::He,
            ProjKind::Flash => GrenadeKind::Flash,
            ProjKind::Decoy => GrenadeKind::Decoy,
            ProjKind::Fire => match round.teams.get(thrower).copied().flatten() {
                Some(Team::CT) => GrenadeKind::Incendiary,
                _ => GrenadeKind::Molotov,
            },
        };
        round.grenades.push(Grenade {
            kind,
            thrower: thrower as u32,
            throw_tick: first.tick,
            end_tick,
            ticks,
            x: xs,
            y: ys,
            z: zs,
            bounces,
            area,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RoundEvent::*;

    fn end(tick: i32, winner: i32) -> RoundEvent {
        End { tick, winner, reason: 9 }
    }

    #[test]
    fn splits_rounds_and_numbers_them() {
        let r = build_rounds(&[
            Start(65), NewMatch(67), FreezeEnd(100), end(500, 2),
            Start(600), FreezeEnd(700), end(1000, 3),
        ]);
        assert_eq!(r.len(), 2);
        // begin_new_match after round_start: the round starts at the later tick.
        assert_eq!((r[0].number, r[0].start_tick, r[0].freeze_end_tick, r[0].end_tick), (1, 67, Some(100), 500));
        assert_eq!(r[0].winner, Some(Team::T));
        assert_eq!((r[1].number, r[1].start_tick), (2, 600));
        assert_eq!(r[1].winner, Some(Team::CT));
    }

    #[test]
    fn new_match_discards_warmup() {
        let r = build_rounds(&[Start(1), FreezeEnd(5), end(10, 2), NewMatch(20), FreezeEnd(30), end(40, 3)]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].start_tick, 20);
    }

    #[test]
    fn round_without_freeze_end_and_unfinished_tail() {
        let r = build_rounds(&[Start(1), end(10, 3), Start(20), FreezeEnd(25)]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].freeze_end_tick, None);
    }

    fn player(index: u32, steam_id: u64) -> Player {
        Player { index, steam_id, name: String::new(), final_team: None }
    }

    fn row(tick: i32, steam_id: u64) -> TickRow {
        TickRow { tick, steam_id, ..Default::default() }
    }

    #[test]
    fn snapshots_fill_arrays_and_carry_position_when_dead() {
        let mut rounds = build_rounds(&[Start(0), FreezeEnd(4), end(8, 2)]);
        let players = [player(0, 10), player(1, 20)];
        let alive = |tick, x: f32, w: &str| TickRow {
            x: Some(x), y: Some(0.0), z: Some(1.5), yaw: Some(90.25), pitch: Some(-10.0),
            hp: Some(100), alive: Some(true), team: Some(3), weapon: Some(w.into()),
            money: Some(800), armor: Some(100), helmet: Some(true), inventory: vec!["knife".into(), w.into()],
            spent: Some(if tick == 4 { 650 } else { 0 }), ..row(tick, 10)
        };
        let rows = vec![
            alive(0, 1.0, "AK-47"), alive(4, 2.0, "AK-47"),
            // tick 8: player 10 has no pawn (dead); player 20 never appears.
            TickRow { alive: Some(false), money: Some(800), ..row(8, 10) },
        ];
        let mut dict = Dict::default();
        fill_snapshots(&mut rounds, &players, &rows, &mut dict);
        let r = &rounds[0];
        assert_eq!(r.ticks, vec![0, 4, 8]);
        assert_eq!(r.teams, vec![Some(Team::CT), None]);
        let t = &r.tracks[0];
        assert_eq!(t.x, vec![10, 20, 20]); // tenths; carried after death
        assert_eq!(t.z, vec![15, 15, 15]);
        assert_eq!(t.yaw[0], 903); // 90.25 -> 902.5 rounds half away from zero
        assert_eq!(t.hp, vec![100, 100, 0]);
        assert_eq!(t.weapon, vec![(0, 0)]);
        assert_eq!(dict.names, vec!["AK-47".to_string(), "knife".to_string()]);
        assert_eq!(t.inventory, vec![InventoryChange(0, 100, 1, vec![1, 0])]);
        assert_eq!(t.spent, 650);
        assert_eq!(t.money, vec![800, 800, 800]);
        // The absent player is dead everywhere but arrays still have full length.
        assert_eq!(r.tracks[1].hp, vec![0, 0, 0]);
        assert_eq!(r.tracks[1].x.len(), 3);
    }

    #[test]
    fn tick_rate_from_game_time() {
        let rounds = build_rounds(&[Start(0), end(1000, 2)]);
        let rows: Vec<TickRow> = [0, 640].iter().map(|&t| TickRow { game_time: Some(100.0 + t as f32 / 128.0), ..row(t, 1) }).collect();
        assert_eq!(derive_tick_rate(&rows, &rounds), 128);
        assert_eq!(derive_tick_rate(&[], &rounds), 64);
    }

    #[test]
    fn events_and_bomb_track() {
        let mut rounds = build_rounds(&[Start(0), FreezeEnd(4), end(40, 2), Start(100), end(120, 3)]);
        let players = [player(0, 10), player(1, 20)];
        let mut dict = Dict::default();
        dict.id("C4 Explosive");
        let mk = |tick, steam, x: f32, hp, inv: &[&str]| TickRow {
            x: Some(x), y: Some(0.0), z: Some(0.0), yaw: Some(0.0), pitch: Some(0.0), hp: Some(hp), alive: Some(hp > 0),
            weapon: Some("knife".into()), inventory: inv.iter().map(|s| s.to_string()).collect(), ..row(tick, steam)
        };
        let mut rows = vec![];
        for (i, t) in (0..=40).step_by(4).enumerate() {
            // Player 10 carries C4 until it dies at tick 16 (snapshot 4); player 20 picks it up at 28.
            let p10_alive = t < 16;
            rows.push(mk(t, 10, 10.0 + t as f32, if p10_alive { 100 } else { 0 }, if p10_alive || i < 4 { &["C4 Explosive"] } else { &[] }));
            rows.push(mk(t, 20, 500.0, 100, if t >= 28 { &["C4 Explosive"] } else { &[] }));
        }
        fill_snapshots(&mut rounds, &players, &rows, &mut dict);
        fill_events(&mut rounds, &players, &[
            MatchEvent::Death { tick: 16, victim: 10, killer: Some(20), assister: None, weapon: "ak47".into(), flags: KILL_HEADSHOT },
            MatchEvent::Hurt { tick: 15, victim: 10, attacker: Some(99), weapon: "ak47".into(), dmg: 100, armor: 0, hp_after: 0 },
            MatchEvent::BombPlanted { tick: 36, player: 20 },
            MatchEvent::BombExploded { tick: 40, player: 20 },
            MatchEvent::BombDropped { tick: 16, player: 10 },
            MatchEvent::Blind { tick: 110, victim: 20, thrower: None, ms: 1500 }, // round 2
            MatchEvent::Blind { tick: 500, victim: 20, thrower: None, ms: 1 }, // after the last round: dropped
        ], &mut dict);
        let r = &rounds[0];
        assert_eq!(r.events.kills, vec![Kill(16, 1, 0, -1, dict.get("ak47").unwrap(), KILL_HEADSHOT)]);
        assert_eq!(r.events.damage[0].1, -1); // unknown attacker -> -1
        assert_eq!((r.bomb.plant_tick, r.bomb.planter, r.bomb.end_type), (Some(36), Some(1), Some(BombEnd::Exploded)));
        assert_eq!(r.bomb.x, Some(5000));
        assert_eq!(r.bomb.drops, vec![BombSpot(16, 0, 260, 0, 0)]);
        // Prefix of ticks up to the plant (snapshot 9 = tick 36): carrier 0, ground, carrier 1.
        assert_eq!(r.bomb.track_x.len(), 10);
        assert_eq!(r.bomb.carrier, vec![(0, 0), (4, -1), (7, 1)]);
        assert_eq!(r.bomb.track_x[5], 220); // lies where its holder last stood (tick 12 -> x 22.0)
        assert_eq!(rounds[1].events.blinds, vec![Blind(110, -1, 1, 1500)]);
    }

    fn pt(tick: i32, entity: i32, steam: u64, kind: ProjKind, pos: [f32; 3]) -> ProjPoint {
        ProjPoint { tick, entity, steam_id: steam, kind, pos }
    }

    /// A lobbed grenade: constant horizontal speed, parabola, bounce off the floor at tick 40.
    fn lob(entity: i32, steam: u64, kind: ProjKind, t0: i32, ticks: i32) -> Vec<ProjPoint> {
        let g = 800.0 / (64.0 * 64.0);
        let (mut z, mut vz, mut x) = (50.0f32, 2.0f32, 0.0f32);
        let mut out = vec![];
        for i in 0..ticks {
            out.push(pt(t0 + i, entity, steam, kind, [x, 0.0, z]));
            x += 5.0;
            vz -= g;
            z += vz;
            if z <= 0.0 {
                z = -z * 0.5;
                vz = -vz * 0.5;
            }
        }
        out
    }

    #[test]
    fn grenades_flight_end_area_and_bounce() {
        let mut rounds = build_rounds(&[Start(0), end(500, 2)]);
        let players = [player(0, 10)];
        rounds[0].teams = vec![Some(Team::CT)];
        let mut pts = lob(7, 10, ProjKind::Smoke, 10, 60);
        // The smoke entity keeps existing (stationary cloud) after its bloom at tick 49.
        pts.extend((70..120).map(|t| pt(t, 7, 10, ProjKind::Smoke, [300.0, 0.0, 0.0])));
        pts.extend(lob(8, 10, ProjKind::Fire, 200, 30));
        let ev = [
            MatchEvent::Detonate { kind: ProjKind::Smoke, tick: 49, entity: 7, thrower: 10, pos: [245.0, 0.0, 0.0] },
            MatchEvent::SmokeExpired { tick: 400, entity: 7 },
            MatchEvent::FireStart { tick: 230, entity: 99, thrower: 10, pos: [150.0, 0.0, 0.0] },
            MatchEvent::FireEnd { tick: 300, entity: 99 },
        ];
        fill_grenades(&mut rounds, &players, &pts, &ev, 64);
        let g = &rounds[0].grenades;
        assert_eq!(g.len(), 2);
        let s = &g[0];
        assert_eq!((s.kind, s.thrower, s.throw_tick, s.end_tick), (GrenadeKind::Smoke, 0, 10, 49));
        assert_eq!(s.ticks.first(), Some(&10));
        assert_eq!(s.ticks.last(), Some(&49)); // exact end point appended
        assert!(s.ticks[..s.ticks.len() - 1].windows(2).all(|w| w[1] - w[0] == 4));
        assert_eq!(s.area, Some(Area { start_tick: 49, end_tick: Some(400), x: 2450, y: 0, z: 0 }));
        assert!(!s.bounces.is_empty(), "floor bounce detected");
        let f = &g[1];
        assert_eq!(f.kind, GrenadeKind::Incendiary); // thrown by a CT
        assert_eq!(f.end_tick, 230);
        assert_eq!(f.area.as_ref().unwrap().end_tick, Some(300));
    }

    #[test]
    fn straight_fall_has_no_bounce() {
        let g = 800.0 / (64.0 * 64.0);
        let mut v = 0.0;
        let mut z = 1000.0;
        let pts: Vec<ProjPoint> = (0..30).map(|t| { let p = pt(t, 1, 1, ProjKind::He, [t as f32 * 4.0, 0.0, z]); v -= g; z += v; p }).collect();
        let refs: Vec<&ProjPoint> = pts.iter().collect();
        assert!(find_bounces(&refs, 64).is_empty());
    }
}

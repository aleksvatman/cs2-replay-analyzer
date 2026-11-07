//! Thin wrapper over the demoparser2 core. Stage 1: open a demo and read the header.

use crate::error::ParseError;
use crate::models::{Match, Player, Team, SCHEMA_VERSION};
use ahash::AHashMap;
use memmap2::MmapOptions;
use parser::first_pass::parser_settings::ParserInputs;
use parser::parse_demo::{Parser, ParsingMode};
use parser::second_pass::parser_settings::create_huffman_lookup_table;
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

/// Match-level info: map, tick rate and the player roster.
pub fn read_match_info(path: &Path) -> Result<Match, ParseError> {
    let file = File::open(path)?;
    // Safety: read-only mapping, the file must not change while parsing.
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let huf = create_huffman_lookup_table();
    let inputs = ParserInputs {
        real_name_to_og_name: AHashMap::default(),
        wanted_players: vec![],
        wanted_player_props: vec!["team_num".into()],
        wanted_other_props: vec![],
        wanted_prop_states: AHashMap::default(),
        wanted_ticks: vec![],
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

    Ok(Match {
        schema_version: SCHEMA_VERSION,
        map: out
            .header
            .as_ref()
            .and_then(|h| h.get("map_name").cloned())
            .unwrap_or_default(),
        // The demo does not expose it directly; CS2 matchmaking and servers run at 64.
        // TODO: derive from tick/game-time once the tick pass exists (stage 4).
        tick_rate: DEFAULT_TICK_RATE,
        players,
        weapons: vec![],
        rounds: vec![],
    })
}

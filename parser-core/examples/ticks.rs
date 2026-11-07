//! Dev helper: dumps tick-pass columns for a few ticks to see names and value types.
use ahash::AHashMap;
use memmap2::MmapOptions;
use parser::first_pass::parser_settings::ParserInputs;
use parser::parse_demo::{Parser, ParsingMode};
use parser::second_pass::parser_settings::create_huffman_lookup_table;

fn cell(c: &parser::second_pass::variants::PropColumn, i: usize) -> String {
    use parser::second_pass::variants::VarVec::*;
    match c.data.as_ref() {
        Some(U32(v)) => format!("{:?}", v[i]),
        Some(Bool(v)) => format!("{:?}", v[i]),
        Some(U64(v)) => format!("{:?}", v[i]),
        Some(F32(v)) => format!("{:?}", v[i]),
        Some(I32(v)) => format!("{:?}", v[i]),
        Some(String(v)) => format!("{:?}", v[i]),
        Some(StringVec(v)) => format!("{:?}", v[i]),
        Some(U32Vec(v)) => format!("{:?}", v[i]),
        Some(U64Vec(v)) => format!("{:?}", v[i]),
        Some(_) => "?".into(),
        None => "-".into(),
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: ticks <demo.dem>");
    let file = std::fs::File::open(path).unwrap();
    let mmap = unsafe { MmapOptions::new().map(&file).unwrap() };
    let huf = create_huffman_lookup_table();
    let props = ["X", "Y", "Z", "pitch", "yaw", "health", "team_num", "active_weapon_name", "balance", "armor_value", "has_helmet", "has_defuser", "inventory", "is_alive"];
    let inputs = ParserInputs {
        real_name_to_og_name: AHashMap::default(),
        wanted_players: vec![],
        wanted_player_props: props.iter().map(|s| parser::maps::FRIENDLY_NAMES_MAPPING.get(*s).map(|o| o.to_string()).unwrap_or(s.to_string())).collect(),
        wanted_other_props: vec!["game_time".to_string()],
        wanted_prop_states: AHashMap::default(),
        wanted_ticks: vec![2000, 2004, 4000],
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
    let out = Parser::new(inputs, ParsingMode::ForceSingleThreaded).parse_demo(&mmap).unwrap();
    let mut ids: Vec<_> = out.df.keys().copied().collect();
    ids.sort();
    let names: Vec<String> = ids.iter().map(|id| out.prop_controller.id_to_name.get(id).cloned().unwrap_or(format!("#{id}"))).collect();
    println!("{}", names.join(" | "));
    let n = out.df[&ids[0]].len();
    for row in 0..n.min(12) {
        let cells: Vec<String> = ids.iter().map(|id| cell(&out.df[id], row)).collect();
        println!("{}", cells.join(" | "));
    }
}

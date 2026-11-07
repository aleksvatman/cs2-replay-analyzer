//! Dev helper: lists round-related game events of a demo (tick + fields).
use ahash::AHashMap;
use memmap2::MmapOptions;
use parser::first_pass::parser_settings::ParserInputs;
use parser::parse_demo::{Parser, ParsingMode};
use parser::second_pass::parser_settings::create_huffman_lookup_table;

fn main() {
    let path = std::env::args().nth(1).expect("usage: events <demo.dem>");
    let file = std::fs::File::open(path).unwrap();
    let mmap = unsafe { MmapOptions::new().map(&file).unwrap() };
    let huf = create_huffman_lookup_table();
    let inputs = ParserInputs {
        real_name_to_og_name: AHashMap::default(),
        wanted_players: vec![],
        wanted_player_props: vec![],
        wanted_other_props: vec![],
        wanted_prop_states: AHashMap::default(),
        wanted_ticks: vec![],
        wanted_events: ["round_start", "round_freeze_end", "round_end", "round_officially_ended", "begin_new_match", "cs_win_panel_match"]
            .iter().map(|s| s.to_string()).collect(),
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
    for e in &out.game_events {
        let f: Vec<String> = e.fields.iter().map(|f| format!("{}={:?}", f.name, f.data)).collect();
        println!("{:>7} {} {}", e.tick, e.name, f.join(" "));
    }
}

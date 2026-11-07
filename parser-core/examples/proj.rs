//! Dev helper: dumps projectile records (grenades in flight) of a demo.
use ahash::AHashMap;
use memmap2::MmapOptions;
use parser::first_pass::parser_settings::ParserInputs;
use parser::parse_demo::{Parser, ParsingMode};
use parser::second_pass::parser_settings::create_huffman_lookup_table;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: proj <demo.dem> [grenades]");
    let grenades = args.next().is_some();
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
        wanted_events: vec![],
        parse_ents: true,
        parse_projectiles: true,
        parse_grenades: grenades,
        parse_weapons: false,
        only_header: false,
        only_convars: false,
        huffman_lookup_table: &huf,
        order_by_steamid: false,
        list_props: false,
        fallback_bytes: None,
    };
    let out = Parser::new(inputs, ParsingMode::ForceSingleThreaded).parse_demo(&mmap).unwrap();
    use parser::first_pass::prop_controller::*;
    use parser::second_pass::variants::VarVec;
    let col = |id| out.df.get(&id).and_then(|c| c.data.as_ref());
    let (Some(VarVec::String(ty)), Some(VarVec::I32(tick)), Some(VarVec::I32(ent)), Some(VarVec::U64(sid))) =
        (col(GRENADE_TYPE_ID), col(TICK_ID), col(ENTITY_ID_ID), col(STEAMID_ID)) else { println!("no columns: {:?}", out.df.keys().collect::<Vec<_>>()); return };
    let f = |id| match col(id) { Some(VarVec::F32(v)) => v.clone(), _ => vec![] };
    let (x, y, z) = (f(GRENADE_X), f(GRENADE_Y), f(GRENADE_Z));
    let mut counts: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for i in 0..ty.len() {
        let e = counts.entry(ty[i].clone().unwrap_or_default()).or_default();
        e.0 += 1;
        if x.get(i).map_or(false, |v| v.is_some()) { e.1 += 1; }
    }
    println!("{} rows; type -> (rows, rows with coords) {:?}", ty.len(), counts);
    let first = (0..ty.len()).find(|&i| x.get(i).map_or(false, |v| v.is_some())).unwrap();
    println!("first entity {:?} type {:?} thrower {:?}", ent[first], ty[first], sid[first]);
    for i in (0..ty.len()).filter(|&i| ent[i] == ent[first]).take(45) {
        println!("{:?} {:?} {:?} {:?}", tick[i], x[i], y[i], z[i]);
    }
}

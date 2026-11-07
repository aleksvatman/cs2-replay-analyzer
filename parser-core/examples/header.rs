use std::path::Path;

fn main() {
    let path = std::env::args().nth(1).expect("usage: header <demo.dem>");
    match parser_core::parse_demo::read_match_info(Path::new(&path)) {
        Ok(m) => println!("{}", serde_json::to_string_pretty(&m).unwrap()),
        Err(e) => eprintln!("error: {e}"),
    }
}

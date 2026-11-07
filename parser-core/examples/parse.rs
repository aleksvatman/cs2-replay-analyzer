//! Parses a demo and writes the replay JSON: `parse <demo.dem> [out.json | out.json.gz]`.
//! Without an output path the JSON goes to stdout. Progress goes to stderr.
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    let mut args = std::env::args().skip(1);
    let demo = args.next().expect("usage: parse <demo.dem> [out.json | out.json.gz]");
    let out = args.next();
    let start = std::time::Instant::now();
    let mut progress = |p: parser_core::Progress| eprintln!("[{:>5.1}s] {:?} {}%", start.elapsed().as_secs_f32(), p.stage, p.percent);
    let m = match parser_core::parse_file(Path::new(&demo), Some(&mut progress)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    let result = match out {
        None => parser_core::write_json(&m, BufWriter::new(std::io::stdout().lock())),
        Some(path) => {
            let file = BufWriter::new(std::fs::File::create(&path).expect("create output"));
            if path.ends_with(".gz") {
                // Fixed level so the reference file is reproducible.
                let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::new(9));
                let r = parser_core::write_json(&m, &mut gz);
                gz.finish().and_then(|mut f| f.flush()).expect("finish output");
                r
            } else {
                parser_core::write_json(&m, file)
            }
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

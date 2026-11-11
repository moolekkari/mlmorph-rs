use mlmorph_spike::Transducer;
use std::io::{BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 2 || args.len() > 3 {
        return Err("usage: spike-lookup data.bin [--generate]".into());
    }
    let bytes = std::fs::read(&args[1])?;
    let transducer = Transducer::from_bytes(&bytes)?;
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        writeln!(out, "> {line}")?;
        let results = if args.get(2).map(String::as_str) == Some("--generate") {
            transducer.generate(&line)
        } else {
            transducer.analyse(&line)
        };
        for result in results {
            writeln!(out, "{result}")?;
        }
    }
    Ok(())
}

use mlmorph::{Analyser, normalize};
use std::hint::black_box;
use std::time::Instant;

#[path = "../src/transducer.rs"]
mod transducer;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: benchmark coverage-file.counts");
    let input = std::fs::read_to_string(path).unwrap();
    let words: Vec<_> = input
        .lines()
        .map(|line| line.split_once('\t').unwrap().0)
        .collect();
    let normalized: Vec<_> = words.iter().map(|word| normalize(word)).collect();
    let bytes = include_bytes!("../data/malayalam.bin");
    let load_start = Instant::now();
    let raw = transducer::Transducer::from_bytes(bytes).unwrap();
    println!(
        "transducer_bytes={} load_ms={:.3}",
        raw.memory_bytes(),
        load_start.elapsed().as_secs_f64() * 1000.0
    );
    let api = Analyser::new().unwrap();
    black_box(raw.generate("കേരളം<np><genitive>"));
    for word in &normalized {
        black_box(raw.analyse(word));
    }
    for (label, public) in [("raw_lookup", false), ("public_api_unweighted", true)] {
        let start = Instant::now();
        let mut count = 0;
        let mut results = 0;
        // Measure one thread for at least one second to reduce timer noise.
        while start.elapsed().as_secs_f64() < 1.0 {
            for (word, normalized) in words.iter().zip(&normalized) {
                results += if public {
                    black_box(api.analyse(black_box(word), false, false).unwrap()).len()
                } else {
                    black_box(raw.analyse(black_box(normalized))).len()
                };
                count += 1;
            }
        }
        println!(
            "{label}: words={count} results={results} seconds={:.6} words_per_second={:.0}",
            start.elapsed().as_secs_f64(),
            count as f64 / start.elapsed().as_secs_f64()
        );
    }
}

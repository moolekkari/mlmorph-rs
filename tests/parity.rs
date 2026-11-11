use mlmorph::{Analyser, Generator};
use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader};
use std::path::Path;

fn check_fixture(path: &Path, mut lookup: impl FnMut(&str) -> Vec<String>) -> usize {
    let mut previous = String::new();
    let mut expected = BTreeSet::new();
    let mut checked = 0;
    for line in BufReader::new(std::fs::File::open(path).unwrap())
        .lines()
        .chain(std::iter::once(Ok("\t".to_string())))
    {
        let line = line.unwrap();
        let (word, analysis) = line.split_once('\t').unwrap();
        if word != previous && !previous.is_empty() {
            let actual: BTreeSet<_> = lookup(&previous).into_iter().collect();
            assert_eq!(actual, expected, "{}: {previous}", path.display());
            expected.clear();
            checked += 1;
        }
        previous = word.to_owned();
        if !analysis.is_empty() {
            expected.insert(analysis.to_owned());
        }
    }
    checked
}

#[test]
fn json_and_format_probe_parity() {
    let analyser = Analyser::new().unwrap();
    let generator = Generator::new().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert!(
        check_fixture(&fixtures.join("analyse.tsv"), |word| analyser
            .analyse(word, false, false)
            .unwrap()
            .into_iter()
            .map(|(a, _)| a)
            .collect())
            >= 477
    );
    assert!(
        check_fixture(&fixtures.join("generate.tsv"), |word| generator
            .generate(word, false)
            .unwrap()
            .into_iter()
            .map(|(a, _)| a)
            .collect())
            >= 417
    );
}

#[test]
#[ignore = "requires full SFST fixtures: run build.sh"]
fn complete_coverage_corpus_parity() {
    let coverage = std::path::PathBuf::from(
        std::env::var("MLMORPH_COVERAGE_FIXTURES").expect("set MLMORPH_COVERAGE_FIXTURES"),
    );
    let analyser = Analyser::new().unwrap();
    let mut files: Vec<_> = std::fs::read_dir(&coverage)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "tsv"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let (mut total, mut raw, mut analysed, mut checked) = (0usize, 0usize, 0usize, 0usize);
    for path in files {
        let counts: HashMap<String, usize> = std::fs::read_to_string(path.with_extension("counts"))
            .unwrap()
            .lines()
            .map(|line| {
                let (word, count) = line.split_once('\t').unwrap();
                (word.to_string(), count.parse().unwrap())
            })
            .collect();
        let cases = check_fixture(&path, |word| {
            let results = analyser.analyse(word, false, false).unwrap();
            let count = counts[word];
            total += count;
            if !results.is_empty() {
                raw += count;
            }
            if !results.is_empty() || !analyser.analyse(word, false, true).unwrap().is_empty() {
                analysed += count;
            }
            results.into_iter().map(|(a, _)| a).collect()
        });
        assert_eq!(cases, counts.len());
        checked += cases;
        println!(
            "{}: {cases} exact set matches",
            path.file_name().unwrap().to_string_lossy()
        );
    }
    let coverage = 100.0 * analysed as f64 / total as f64;
    println!(
        "coverage: {analysed}/{total} ({coverage:.4}%), raw FST: {raw}, distinct cases: {checked}"
    );
    assert!(coverage >= 57.25);
}

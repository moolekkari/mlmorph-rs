use mlmorph::{Analyser, Generator, check_foreign_word, normalize};
use proptest::strategy::Strategy;

#[test]
fn public_api_and_shared_loader() {
    let analyser = Analyser::new().unwrap();
    assert!(
        analyser
            .analyse("കേരളത്തിന്റെ", true, false)
            .unwrap()
            .iter()
            .any(|(a, _)| a == "കേരളം<np><genitive>")
    );
    assert!(
        analyser
            .analyse("വനങ്ങളിൽ", false, false)
            .unwrap()
            .iter()
            .any(|(a, _)| a.starts_with("വനം"))
    );
    assert!(
        analyser
            .analyse("computer", false, false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        analyser.analyse("computer", false, true).unwrap(),
        [("computer<fw>".to_string(), 0)]
    );
    assert_eq!(check_foreign_word("computer"), 1);
    assert_eq!(check_foreign_word("കഅക"), 1);
    assert_eq!(check_foreign_word("കറ്റ്ക"), 1);
    assert_eq!(normalize("ണ്‍"), "ൺ");
    assert!(
        Generator::new()
            .unwrap()
            .generate("കേരളം<np><genitive>", true)
            .unwrap()
            .iter()
            .any(|(word, _)| word == "കേരളത്തിന്റെ")
    );
    fn send_sync<T: Send + Sync>(_: &T) {}
    send_sync(&analyser);
    send_sync(&Generator::new().unwrap());
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                assert!(
                    !Analyser::new()
                        .unwrap()
                        .analyse("വനം", false, false)
                        .unwrap()
                        .is_empty()
                )
            });
        }
    });
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(10_000))]

    #[test]
    fn arbitrary_unicode_inputs_never_panic(text in proptest::prop_oneof![
        proptest::collection::vec(proptest::prelude::any::<char>(), 0..128)
            .prop_map(|chars| chars.into_iter().collect::<String>()),
        "[\\u{0D00}-\\u{0D7F}\\u{200C}\\u{200D}<>:-]{0,128}",
    ]) {
        let analyser = Analyser::new().unwrap();
        let generator = Generator::new().unwrap();
        let results = analyser.analyse(&text, false, false).unwrap();
        proptest::prop_assert!(results.windows(2).all(|pair| pair[0].0 < pair[1].0));
        let generated = generator.generate(&text, false).unwrap();
        proptest::prop_assert!(generated.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
}

#[test]
fn upstream_analyser_generator_expectations() {
    let analyser = Analyser::new().unwrap();
    let generator = Generator::new().unwrap();
    let mut count = 0;
    for line in include_str!("fixtures/upstream.tsv").lines() {
        let (word, analysis) = line.split_once('\t').unwrap();
        let analyses = analyser.analyse(word, true, true).unwrap();
        assert!(
            analyses.iter().any(|(result, _)| result == analysis),
            "{word}: missing {analysis}"
        );
        let generated = generator.generate(analysis, true).unwrap();
        assert!(
            generated.iter().any(|(result, _)| result == word),
            "{analysis}: missing {word}"
        );
        assert!(analyses.windows(2).all(|p| p[0].1 <= p[1].1));
        assert!(generated.windows(2).all(|p| p[0].1 <= p[1].1));
        count += 1;
    }
    assert_eq!(count, 474);
}

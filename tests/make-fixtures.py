"""Generate independent oracles with SFST's CLI, without its Python bindings."""
from collections import Counter
import json
from pathlib import Path
import re
import runpy
import subprocess
import sys

from spike import lookup


def tokens(text):
    # These are the expressions from upstream/python/mlmorph/utils.py.
    words = re.split(r'[-+#\.\s?!\(\)\[\]:,;]', text)
    strip_pattern = r"^[ '?|/!@#$%^&*()_+=`{}\[\]\u200c]+|[ '?|/!@#$%^&*()_+=`{}\[\]\u200c]+$"
    return [re.sub(strip_pattern, "", word) for word in words]


def write_fixture(path, inputs, results):
    with path.open("w") as output:
        for word, analyses in zip(inputs, results):
            assert "\t" not in word and "\n" not in word
            for analysis in sorted(analyses) or [""]:
                output.write(f"{word}\t{analysis}\n")


def main():
    grammar = Path(sys.argv[1]).resolve()
    fixtures = Path(sys.argv[2])
    fixtures.mkdir(parents=True, exist_ok=True)
    normalize = runpy.run_path("upstream/python/mlmorph/normalizer.py")["normalize"]
    foreign = runpy.run_path("upstream/python/mlmorph/foreign_word_detector.py")["check_foreign_word"]
    cases = json.loads(Path("upstream/tests/tests.json").read_text())
    for direction, field in [("analyse", "word"), ("generate", "analysis")]:
        reference = fixtures / f"{direction}.compact"
        command = ["fst-compact"] + (["-s"] if direction == "generate" else [])
        subprocess.run(command + [str(grammar), str(reference)], check=True)
        inputs = list(dict.fromkeys(case[field] for case in cases))
        inputs += [word for word in [":", "\\:", "<", ">", ":<punct>", "computer", "കേരളത്തിന്റെ", "വനങ്ങളിൽ"] if word not in inputs]
        results = lookup(["fst-infl2", "-q", str(reference)], [normalize(word) for word in inputs])
        write_fixture(fixtures / f"{direction}.tsv", inputs, results)
    with (fixtures / "upstream.tsv").open("w") as output:
        for case in cases:
            if not case.get("skip"):
                output.write(f'{case["word"]}\t{case["analysis"]}\n')
    coverage = fixtures / "coverage"
    coverage.mkdir(exist_ok=True)
    summary = []
    reference = fixtures / "analyse.compact"
    for file in sorted(Path("upstream/tests/coverage").glob("*")):
        if not file.is_file():
            continue
        counts = Counter(word for word in tokens(file.read_text())
                         if len(word.strip()) > 1 and re.search(r"[\u0D00-\u0D7F\u200C-\u200D]+", word))
        inputs = sorted(counts)
        results = lookup(["fst-infl2", "-q", str(reference)], [normalize(word) for word in inputs])
        write_fixture(coverage / (file.name + ".tsv"), inputs, results)
        with (coverage / (file.name + ".counts")).open("w") as output:
            for word in inputs:
                output.write(f"{word}\t{counts[word]}\n")
        raw = sum(counts[word] for word, analyses in zip(inputs, results) if analyses)
        analysed = sum(counts[word] for word, analyses in zip(inputs, results) if analyses or foreign(normalize(word)))
        row = {"file": file.name, "tokens": sum(counts.values()), "distinct": len(inputs),
               "raw_analysed": raw, "analysed_with_foreign_fallback": analysed}
        summary.append(row)
        print(json.dumps(row), flush=True)
    totals = {key: sum(row[key] for row in summary) for key in summary[0] if key != "file"}
    totals["coverage_percent"] = 100 * totals["analysed_with_foreign_fallback"] / totals["tokens"]
    assert totals["coverage_percent"] >= 57.25
    (fixtures / "coverage-summary.json").write_text(json.dumps({"files": summary, "total": totals}, indent=2) + "\n")
    print(json.dumps(totals), flush=True)


if __name__ == "__main__":
    assert tokens(" ആന,മരം-പൂവ്\t'വനം'\u200c ") == ["", "ആന", "മരം", "പൂവ്", "വനം", ""]
    main()

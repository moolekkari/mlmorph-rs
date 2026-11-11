"""Phase 1 oracle: invoke SFST CLI, never its Python bindings or C++ sources."""
import gzip
import json
from pathlib import Path
import subprocess
import sys


def lookup(command, inputs):
    result = subprocess.run(command, input="\n".join(inputs) + "\n", text=True,
                            capture_output=True, check=True, timeout=120)
    blocks = []
    for line in result.stdout.splitlines():
        if line.startswith("> "):
            blocks.append((line[2:], set()))
        elif line.startswith("no result for "):
            assert blocks and line == "no result for " + blocks[-1][0]
        elif line:
            assert blocks, f"unexpected CLI output: {line!r}"
            blocks[-1][1].add(line)
    assert [word for word, _ in blocks] == inputs, "CLI input/output framing changed"
    return [outputs for _, outputs in blocks]


def main():
    grammar = Path(sys.argv[1]).resolve()
    cases = json.loads(Path("upstream/tests/tests.json").read_text())
    summary = {}
    for direction, field, reference in [
        ("analyse", "word", grammar / "malayalam.a"),
        ("generate", "analysis", Path("build/generation.a")),
    ]:
        inputs = [case[field] for case in cases]
        expected = lookup(["fst-infl", "-q", str(reference)], inputs)
        args = ["target/release/spike-lookup", "build/malayalam.bin"]
        if direction == "generate":
            args.append("--generate")
        actual = lookup(args, inputs)
        mismatches = []
        with Path(f"build/{direction}.tsv").open("w") as fixtures:
            for word, outputs in zip(inputs, expected):
                for output in sorted(outputs) or [""]:
                    fixtures.write(f"{word}\t{output}\n")
        for index, (word, want, got) in enumerate(zip(inputs, expected, actual)):
            if want != got:
                mismatches.append({"index": index, "input": word,
                                   "missing": sorted(want - got), "extra": sorted(got - want)})
        checked = [(case, outputs) for case, outputs in zip(cases, actual) if not case.get("skip")]
        expected_field = "analysis" if direction == "analyse" else "word"
        membership = sum(case[expected_field] in outputs for case, outputs in checked)
        summary[direction] = {
            "cases": len(inputs), "distinct_inputs": len(set(inputs)),
            "exact_matches": len(inputs) - len(mismatches),
            "parity_percent": 100 * (len(inputs) - len(mismatches)) / len(inputs),
            "analysis_count": sum(map(len, actual)),
            "upstream_expected_membership": membership,
            "upstream_non_skipped_cases": len(checked), "mismatches": mismatches,
        }
    # Verify column direction with independent, recognizable examples.
    examples = ["കേരളത്തിന്റെ", "വനങ്ങളിൽ", "computer"]
    oracle = lookup(["fst-infl", "-q", str(grammar / "malayalam.a")], examples)
    actual = lookup(["target/release/spike-lookup", "build/malayalam.bin"], examples)
    assert actual == oracle
    assert "കേരളം<np><genitive>" in actual[0]
    assert "വനം<n><pl><locative>" in actual[1]
    assert actual[2] == set()
    summary["examples"] = dict(zip(examples, map(sorted, actual)))
    # Keep observed formatting limitations visible without overstating corpus parity.
    probes = [":", "\\:", "<", ">"]
    oracle = lookup(["fst-infl", "-q", str(grammar / "malayalam.a")], probes)
    actual = lookup(["target/release/spike-lookup", "build/malayalam.bin"], probes)
    summary["format_probes"] = [
        {"input": word, "sfst": sorted(want), "rust": sorted(got)}
        for word, want, got in zip(probes, oracle, actual)
    ]
    binary = Path("build/malayalam.bin").read_bytes()
    summary["transducer"] = dict(zip(
        ["states", "arcs", "symbols_including_epsilon"],
        [int.from_bytes(binary[i:i + 4], "little") for i in [8, 12, 16]],
    ))
    compressed = gzip.compress(binary, compresslevel=9, mtime=0)
    Path("build/malayalam.bin.gz").write_bytes(compressed)
    summary["sizes_bytes"] = {
        "att": Path("build/malayalam.att").stat().st_size,
        "binary": len(binary), "gzip_9": len(compressed),
    }
    Path("build/parity.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    assert all(summary[d]["exact_matches"] == len(cases) for d in ["analyse", "generate"])


if __name__ == "__main__":
    main()

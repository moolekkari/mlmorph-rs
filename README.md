# mlmorph-rs

An MIT-licensed, pure-Rust packaging of [SMC’s mlmorph](https://gitlab.com/smc/mlmorph), by Santhosh Thottingal. SFST compiles the upstream MIT grammar during the build and supplies independent test fixtures. No SFST implementation, C++ library, or compiled SFST transducer is linked or shipped in the crate.

The exported package is named `mlmorph-rs`; its library remains `mlmorph`. `Analyser`, `Generator`, `normalize`, and `check_foreign_word` retain their public signatures. `create_sfst()` is removed. The default `cli` feature installs the `mlmorph` binary; disable default features for library-only use. This package has not been published.

```toml
mlmorph = { package = "mlmorph-rs", path = "./out", default-features = false }
```

```rust
use mlmorph::Analyser;
let analyser = Analyser::new()?;
let analyses = analyser.analyse("കേരളത്തിന്റെ", false, false)?;
```

The last argument disables the upstream foreign-word fallback, giving strict grammar results. With the fallback enabled, unknown foreign words can return a `<fw>` analysis.

## Build and verify

```sh
nix develop -c ./build.sh
```

This initializes the submodule, builds a fresh scratch copy of the grammar, dumps AT&T text, converts the data, exports the crate to `out/`, and applies the patches with `git apply`. It then runs unit tests, integration tests, doctests, both CLI feature configurations, Clippy with warnings denied, full corpus parity, the benchmark, and `cargo package` verification. The GitHub Actions workflow runs the same command; it never publishes.

The submodule is pinned to `v1.4.3` (`7442b123a8201551b94a9cc2957b9c40699c4c8d`). Nothing is edited in `upstream/`, including generated files. Old exports are preserved under `build/export.*`; the final package is `target/export/package/mlmorph-rs-1.4.3.crate`.

Both the default dev shell and `nix develop .#ci` use the pinned `flake.lock`. Nixpkgs lacks SFST, so the flake builds the CLI tools from the author’s official 1.4.7g archive with a fixed SHA-256. Only its build files and CLI documentation were inspected; no SFST C++ implementation or crate source was read.

Python uses only its standard library to tokenize the corpus, read JSON, run SFST’s CLI, and write fixtures. Its tokenization uses the expressions from upstream’s `utils.py`; normalization and foreign-word detection are loaded directly from the upstream Python files. No SFST Python bindings are needed.

SFST CLI fixtures are generated from the same grammar as the embedded binary. The compact CLI transducer is used for analysis and its switched-level counterpart for generation. Fixtures use `word<TAB>analysis`; an empty second field records no result. The complete coverage fixtures stay under `build/fixtures/coverage` and are excluded from the package. Small JSON-case fixtures are shipped so ordinary `cargo test` runs the upstream expectations and exact-set checks without SFST. The large corpus test is marked ignored for ordinary crate consumers; `build.sh` explicitly runs it in release mode.

For a standalone recheck after building:

```sh
MLMORPH_COVERAGE_FIXTURES="$PWD/build/fixtures/coverage" cargo test --manifest-path out/Cargo.toml --release --test parity -- --ignored --nocapture
cargo run --manifest-path out/Cargo.toml --release --example benchmark -- build/fixtures/coverage/1.txt.counts
```

## Measurements

| Measurement | Result |
| --- | ---: |
| States / arcs / symbols including epsilon | 606,431 / 2,099,458 / 207 |
| Final states | 776 |
| AT&T / binary bytes | 44,906,468 / 19,299,644 |
| Phase 1 gzip level 9 bytes | 5,846,754 |
| JSON analysis and generation parity | 484 / 484 each, 100% |
| Non-skipped upstream expected-result membership | 474 / 474 each direction |
| Coverage files / token occurrences | 34 / 1,403,201 |
| Distinct per-file corpus inputs with exact set parity | 1,375,973 / 1,375,973 |
| Raw grammar matches in corpus | 669,339 |
| Matches with upstream foreign-word fallback | 805,022 (57.3704%) |
| Loaded transducer, including embedded data and retained heap buffers | 19,311,356 bytes |
| Raw lookup, one thread | 189,626 words/sec |
| Public API, unweighted and without foreign fallback | 168,305 words/sec |

The benchmark used all 1,537 distinct tokens from `coverage/1.txt`, warmed the lookup, and repeated the file for at least one second per measurement on an AMD Ryzen AI 7 PRO 350 with pinned rustc 1.91.1. Raw lookup uses normalized words; the public API includes upstream normalization. Memory excludes per-lookup scratch space and allocator bookkeeping; it is not process RSS. The result is below the rough 200k words/sec target; no further optimization was attempted. Timing varies with system load.

Coverage counts include repeated tokens, as upstream’s coverage test does. Exact-set comparisons disable foreign fallback; the separate coverage calculation enables it. A two-line patch corrects doubled backslashes in two upstream Rust foreign-word regexes to match Python. Without that fix, raw lookup still matches every corpus input, but coverage drops to 56.7148%. This is an intentional behavior correction in `check_foreign_word`, with no signature change.

The loader validates the header, lengths, UTF-8, CSR offsets, arc bounds, and ordering. It borrows the embedded binary and allocates three lookup tables. `OnceLock` shares one validated transducer across threads. Lookup uses binary search on surface-sorted arcs and an explicit DFS stack, avoiding call-stack exhaustion. Generation scans outgoing arcs. Longest-match tokenization preserves tags as single symbols; results are deduplicated.

Robustness uses proptest 1.9.0 as a dev-dependency only, with automatic shrinking of failing inputs. Each run checks 10,000 arbitrary byte buffers, 10,000 mutations of valid binary data, and 10,000 Unicode inputs against the real analysis and generation APIs. String generation combines arbitrary Unicode with Malayalam and tag characters. Inputs are bounded to 4,095 bytes or 128 characters; these tests are not a proof for unbounded inputs. Focused tests also cover every truncation of a small binary, invalid indices, epsilon cycles, reconverging paths, and a 100,000-character consuming path. The suite passes with CLI enabled and disabled on Rust 1.85.0 and the pinned Rust 1.91.1. Runtime dependencies are unchanged.

## Format and compatibility

The observed tab-separated arc format is `source target analysis surface`. Epsilon is `<>`; final records contain only a state number. Tags such as `<n>` and `<genitive>` are single symbols. SFST escapes colon as `\:` in printed output but matches literal `:` in input; the backend now matches this behavior in both directions. Both sides’ epsilon-only graphs are acyclic in the pinned grammar. Path-local cycle detection preserves different outputs at reconverging paths. Productive epsilon cycles in a future grammar would be cut; revisit that policy if such cycles appear.

The binary uses a versioned header, UTF-8 symbol table, final-state bitset, CSR offsets, and 8-byte arcs (u32 target, u16 analysis, u16 surface), all integers little-endian. There is no SFST binary decoder.

The minimum Rust version is 1.85, required by upstream’s edition 2024; both feature configurations have been tested on 1.85.0. The pinned Nix shell uses 1.91.1, matching Meilisearch’s toolchain checked during Phase 1. Runtime dependencies are only `regex` and `lazy_static`, with optional `clap` for the CLI.

## Re-sync and follow-ups

Choose the new upstream tag explicitly, update the submodule pin and commit checks in `build.sh` and `spike.sh`, and review/rebase the small patch series. Keep new backend and build-tool code in `src/`. Rerun the complete pipeline and inspect parity, coverage, format, and performance results. `spike.sh` remains available for the smaller Phase 1 experiment; its historical results are in `tests/phase1-results.json` and `tests/phase1-att-observations.json`.

Morpheme surface spans (`analyse_spans`), a normalization speedup, and charabia integration remain separate follow-ups. Publishing requires a release tag and explicit approval. No publish step is automated.

# mlmorph-rs

An MIT-licensed, pure-Rust lookup backend for [SMC's mlmorph](https://gitlab.com/smc/mlmorph), by Santhosh Thottingal. The upstream grammar and copyright remain upstream's. SFST is used only as a build and test tool; no SFST implementation is linked or shipped in the Rust binaries.

**Status: Phase 1 spike complete.** The root Cargo package is unpublished, has no dependencies, and exposes only the experimental transducer. The source-compatible `mlmorph` library export, patches, release pipeline, and CI belong to Phase 2.

## Reproduce Phase 1

```sh
git submodule update --init
nix develop -c ./spike.sh
```

The submodule is pinned to `v1.4.3` (`7442b123a8201551b94a9cc2957b9c40699c4c8d`). The script exports a fresh scratch grammar under `build/`, compiles it, dumps it with `fst-print`, converts the dump, and compares the Rust results with `fst-infl`. It tests generation using a separate transducer with its levels switched by `fst-compiler-utf8 -s`. Python uses only its standard library to read JSON, invoke the CLIs, and compare sets; no SFST Python bindings are used.

`build/` contains generated AT&T, binary, gzip, CLI fixtures (`analyse.tsv` and `generate.tsv`), and `parity.json`. An empty analysis field in a fixture means no results. A repeat run creates another scratch grammar directory. The pinned submodule is never edited, including during grammar compilation.

Both `nix develop` and `nix develop .#ci` provide the same environment. Nixpkgs lacks SFST, so the flake builds the CLI tools from the author's official 1.4.7g release archive, verified by SHA-256. Only its Makefile and CLI documentation were inspected; no SFST C++ implementation or crate source was read. Nixpkgs and Rust are pinned by `flake.lock`.

## Phase 1 measurements

| Measurement | Result |
| --- | ---: |
| States | 606,431 |
| Arcs | 2,099,458 |
| Symbols, including epsilon | 207 |
| Final states | 776 |
| AT&T bytes | 44,906,468 |
| Binary bytes | 19,299,644 |
| Gzip bytes, level 9, pinned Python | 5,846,754 |
| Exact analysis set parity | 484 / 484 (100%) |
| Exact generation set parity | 484 / 484 (100%) |
| Upstream non-skipped expected-result membership, each direction | 474 / 474 |

These are all 484 JSON entries, including skipped entries; there are 477 distinct words and 417 distinct analysis inputs. The comparison covers raw transducer results, before upstream normalization, foreign-word fallback, or weighting. See [the saved results](tests/phase1-results.json).

Observed dump format: tab-separated `source target analysis surface` arcs, `<>` for epsilon, and a single state number for a final state (no weight). Tags such as `<n>` and `<genitive>` are single symbols. Column 4 is the input side for analysis, confirmed against the SFST CLI. Column 3 is the input side for generation. Surface labels contain no grammatical tags. A scan of each side's epsilon-only graph found no cycles (134,654 analysis-epsilon arcs and 180,553 surface-epsilon arcs).

The lookup and SFST both return `കേരളം<np><genitive>` for `കേരളത്തിന്റെ`, `വനം<n><pl><locative>` for `വനങ്ങളിൽ`, and no analysis for `computer`.

The binary uses a versioned header, length-prefixed UTF-8 symbols, a final-state bitset, CSR offsets, and 8-byte arcs (u32 target, u16 analysis, u16 surface), all integers little-endian. Arcs are sorted by state and surface symbol. The loader borrows the binary and allocates a symbol-reference table. The spike scans outgoing arcs; binary search is deferred.

**Known format limitation:** the dump spells colon `\:`. SFST accepts `:` and emits `\:<punct>`; the spike currently matches the printed spelling literally. The extra punctuation probes record this mismatch. Colon escaping and CLI tokenization need resolving in Phase 2; the 484-entry parity claim does not cover this case.

The spike also uses recursive DFS and path-local cycle detection. This preserves distinct outputs at reconverging states, unlike a global visited set. Productive epsilon cycles would be cut, and very long inputs can exhaust the stack. This is experimental code, not a validated arbitrary-input implementation.

Rust baseline: `rust-version = "1.85"`, required by upstream's edition 2024. Verification used pinned rustc 1.91.1, which matches [Meilisearch's current toolchain](https://github.com/meilisearch/meilisearch/blob/main/rust-toolchain.toml). Rust 1.85 itself has not been tested. The requested `fst-print --help; cargo --version` shell check exits successfully, but this SFST version treats `--help` as a filename; real dumping and CLI parity provide the functional check.

## Re-sync and roadmap

For an upstream update, explicitly choose the new tag, update the submodule pin and `spike.sh` commit check, then rerun the spike and review format and parity changes. Phase 2 will introduce the minimal patches against a copied `upstream/rust`, keeping new backend code here in `src/`.

Phase 2 still needs the compatible API export, embedded OnceLock loader, optional CLI, complete coverage-corpus parity, robustness tests, and throughput/memory measurements. No property-test dependency has been added. Morpheme surface spans (`analyse_spans`), a normalization speedup, and charabia integration remain separate follow-ups. Publishing, tags, and opening anything on SMC's GitLab require explicit approval.

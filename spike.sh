#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
git submodule update --init
test "$(git -C upstream rev-parse HEAD)" = 7442b123a8201551b94a9cc2957b9c40699c4c8d
mkdir -p build
grammar_dir=$(mktemp -d "$PWD/build/grammar.XXXXXX")
git -C upstream archive HEAD | tar -x -C "$grammar_dir"
make -C "$grammar_dir" malayalam.a > build/grammar.log 2>&1
fst-print "$grammar_dir/malayalam.a" > build/malayalam.att
cargo test --offline
cargo clippy --offline --all-targets -- -D warnings
cargo build --release --offline
target/release/att2bin build/malayalam.att build/malayalam.bin
# Switch levels in the CLI to obtain an independent generation oracle.
printf '"<%s/malayalam.a>"\n' "$grammar_dir" > build/generation.fst
fst-compiler-utf8 -q -s build/generation.fst build/generation.a
python3 tests/spike.py "$grammar_dir"
git -C upstream diff --exit-code

#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
git submodule update --init
test "$(git -C upstream rev-parse HEAD)" = 7442b123a8201551b94a9cc2957b9c40699c4c8d
mkdir -p build
# Keep generated grammar files outside the pinned submodule.
grammar_dir=$(mktemp -d "$PWD/build/grammar.XXXXXX")
git -C upstream archive HEAD | tar -x -C "$grammar_dir"
make -C "$grammar_dir" malayalam.a > build/grammar.log 2>&1
fst-print "$grammar_dir/malayalam.a" > build/malayalam.att
cargo test
cargo clippy --offline --all-targets -- -D warnings
cargo build --release --offline
target/release/att2bin build/malayalam.att build/malayalam.bin
python3 tests/make-fixtures.py "$grammar_dir/malayalam.a" build/fixtures
./export.sh
export CARGO_TARGET_DIR="$PWD/target/export"
export MLMORPH_COVERAGE_FIXTURES="$PWD/build/fixtures/coverage"
cargo test --manifest-path out/Cargo.toml
cargo test --manifest-path out/Cargo.toml --no-default-features
cargo clippy --manifest-path out/Cargo.toml --all-targets --all-features -- -D warnings
cargo clippy --manifest-path out/Cargo.toml --no-default-features -- -D warnings
cargo test --manifest-path out/Cargo.toml --release --test parity -- --ignored --nocapture
cargo run --manifest-path out/Cargo.toml --release --example benchmark -- build/fixtures/coverage/1.txt.counts | tee build/benchmark.txt
cargo package --manifest-path out/Cargo.toml --allow-dirty
git -C upstream diff --exit-code

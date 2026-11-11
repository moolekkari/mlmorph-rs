#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
if test -e out; then
    previous=$(mktemp -d "$PWD/build/export.XXXXXX")
    mv out "$previous/out"
fi
mkdir -p out/src out/data out/tests/fixtures out/examples
cp -R upstream/rust/. out/
cp src/*.rs out/src/
for patch in patches/*.patch; do
    git apply --directory=out --check "$patch"
    git apply --directory=out "$patch"
done
cp build/malayalam.bin out/data/malayalam.bin
cp LICENSE out/LICENSE.md
cp tests/api.rs tests/parity.rs out/tests/
cp build/fixtures/analyse.tsv build/fixtures/generate.tsv build/fixtures/upstream.tsv out/tests/fixtures/
cp tests/benchmark.rs out/examples/benchmark.rs

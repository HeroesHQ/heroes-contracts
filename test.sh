#!/bin/bash
set -e
cd "$(dirname $0)"
sh ./build_all.sh

for d in */Cargo.toml ; do
    d=$(dirname "$d");
    echo "Testing $d";
    cd "$d"
    cargo test -- --nocapture
    cd ..
done

cargo run --example integration-tests

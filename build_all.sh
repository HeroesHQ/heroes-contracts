#!/bin/bash
set -e
cd "$(dirname $0)"

for d in */Cargo.toml ; do
    d=$(dirname "$d");
    echo "Building $d";
    sh ./$d/build.sh
done

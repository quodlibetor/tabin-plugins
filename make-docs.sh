#!/bin/bash

set -e
set -x

cargo build
cd make-docs
cargo build
cd ..
./make-docs/target/debug/make-docs > src/scripts.rs

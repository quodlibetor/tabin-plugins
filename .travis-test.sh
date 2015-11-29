#!/bin/bash
set -ev

cargo build --verbose
cd turbine-plugins
cargo test --verbose

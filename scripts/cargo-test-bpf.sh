#!/usr/bin/env bash

set -e
cd "$(dirname "$0")/.."

source ./scripts/rust-version.sh stable
source ./scripts/solana-version.sh

export RUSTFLAGS="-D warnings"
export RUSTBACKTRACE=1

set -x

cargo +"$rust_stable" build-bpf
cargo +"$rust_stable" test-bpf -- --nocapture

exit 0

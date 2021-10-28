#!/usr/bin/env bash

set -e

source scripts/rust-version.sh stable
source scripts/solana-version.sh install

set -x

cargo --version
cargo install rustfilt || true
cargo install honggfuzz --version=0.5.52 --force || true

cargo +"$rust_stable" build-bpf --version

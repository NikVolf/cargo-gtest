#!/bin/sh

# Local installation substitute for `cargo install`

cargo build --release
cp target/release/cargo-gtest $HOME/.cargo/bin

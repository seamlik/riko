sample:
	cargo run --bin cargo-riko
	rustfmt target/riko/*.rs
	cargo build --package riko_sample

verify: sample
	cargo fmt -- --check
	cargo test
	gradle check

install:
	cargo install --path macro
	gradle publishToMavenLocal
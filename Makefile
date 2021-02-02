.PHONY: sample
sample:
	cargo run --bin cargo-riko
	rustfmt target/riko/*.rs
	cargo build --package riko_sample

.PHONY: verify
verify: sample
	cargo fmt -- --check
	cargo test
	gradle check

.PHONY: install
install:
	cargo install --path cli
	gradle publishToMavenLocal
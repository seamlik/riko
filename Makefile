.PHONY: sample
sample:
	cargo run --bin cargo-riko
	rustfmt target/riko/*.rs
	cargo -Z unstable-options build --package riko_sample --config profile.dev.lto=true

.PHONY: verify
verify: sample
	cargo fmt -- --check
	cargo test
	gradle check

.PHONY: install
install:
	cargo install --path cli
	gradle publishToMavenLocal
.PHONY: build
build:
	cargo build -p riko
	cargo run --bin cargo-riko
	rustfmt target/riko/*.rs
	cargo build
	gradle assemble

verify: build
	cargo fmt -- --check
	cargo test
	gradle check

install:
	cargo install --path macro
	gradle publishToMavenLocal
build:
	cargo build -p riko
	cargo run --bin cargo-riko
	cargo build
	gradle assemble

test: build
	cargo fmt
	cargo test
	gradle check

install:
	cargo install --path macro
	gradle publishToMavenLocal
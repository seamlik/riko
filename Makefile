build:
	cargo build -p riko
	cargo run --bin cargo-riko
	cargo build
	mvn -Dmaven.test.skip=true package

test: build
	cargo fmt
	cargo test
	mvn test

install:
	cargo install --path macro
	mvn install
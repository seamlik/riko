build:
	cargo build
	mvn -Dmaven.test.skip=true package

test:
	cargo build -p riko
	cargo run --bin cargo-riko
	cargo fmt
	cargo test
	mvn test
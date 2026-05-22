.PHONY: all build check clippy fmt test release clean

all: check

build:
	cargo build --all-targets --all-features

check: fmt clippy test build

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all --check

test:
	cargo test --all-features

release:
	cargo build --release --all-targets --all-features

clean:
	cargo clean

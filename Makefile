.PHONY: check test clippy doc fmt fmt-check clean

# Comprehensive verification — run before claiming work complete.
check: fmt-check clippy test doc

test:
	cargo test --workspace

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# RUSTDOCFLAGS=-D warnings catches broken intra-doc links and other rustdoc
# warnings; cargo doc otherwise only prints them and exits 0.
doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

clean:
	cargo clean

check:
    cargo +nightly fmt --all
    cargo build
    cargo clippy --all-targets
    cargo test

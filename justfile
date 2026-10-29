check:
    cargo fmt --all
    cargo build
    cargo clippy --all-targets
    cargo test

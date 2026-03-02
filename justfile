dev:
    just fmt
    just lint
    just test

fmt *ARGS:
    cargo fmt {{ ARGS }}

lint *ARGS:
    cargo clippy --all-features --all-targets {{ ARGS }}

test *ARGS:
    cargo test --all-features {{ ARGS }}

doc:
    cargo doc --all-features --no-deps --open

ci:
    just fmt -- --check
    just lint -- -D warnings
    just test

install:
    cargo install --path . --all-features

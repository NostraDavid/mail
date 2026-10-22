set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list --unsorted

allow:
    direnv allow

shell:
    devenv shell

setup:
    cargo fetch --locked

update:
    devenv update
    cargo update

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

test:
    cargo test --workspace --locked

check: fmt-check lint test

build:
    cargo build --workspace --locked

run:
    cargo run -p mail --locked

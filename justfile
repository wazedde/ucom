# Unity Commander Justfile
# Usage: `just <task>` or `just --list`

# Run the CLI with optional args: `just run -- help`
[group('run')]
run *args:
    cargo run -- {{ args }}

# Build the project in debug mode
[group('build')]
build:
    cargo build

# Build the project in release mode
[group('build')]
release:
    cargo build --release

# Fast compile check (no outputs)
[group('build')]
check:
    cargo check

# Run the test suite
[group('test')]
test:
    cargo test

# Run tests with all features enabled
[group('test')]
test-all:
    cargo test --all-features

# Lint with Clippy (warnings are errors)
[group('clippy')]
clippy:
    cargo clippy -- -D warnings

# Lint all targets and features
[group('clippy')]
clippy-all:
    cargo clippy --all-targets --all-features -- -D warnings

# Lint with Clippy pedantic (warnings are errors)
[group('clippy')]
clippy-pedantic:
    cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic

# Check formatting without writing changes
[group('format')]
fmt:
    cargo fmt -- --check

# Format the code in place
[group('format')]
fmt-fix:
    cargo fmt

# Install the CLI from the local path
[group('install')]
install:
    cargo install --path .

# Run all CI checks (fmt, clippy, test)
[group('workflow')]
ci:
    cargo fmt -- --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test

# Watch for changes and run checks (requires cargo-watch)
[group('workflow')]
watch:
    cargo watch -x check -x test

# Run tests with output shown
[group('test')]
test-verbose:
    cargo test -- --nocapture

# Clean build artifacts
[group('build')]
clean:
    cargo clean

# Clear ucom cache directory
[group('unity')]
cache-clear:
    cargo run -- cache clear

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

# Build and test test-projects/minimal with a real Unity 2021.3 (needs that editor installed).
# The Unity editor must not have the project open.
[group('unity')]
real-unity:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build -q
    ucom=target/debug/ucom
    project=test-projects/minimal
    target={{ if os() == "macos" { "macos" } else if os() == "windows" { "win64" } else { "linux64" } }}
    # Builds go to a temporary directory, so player builds do not pile up in the checkout.
    out=$(mktemp -d)
    trap 'rm -rf "$out"; rm -f "$project"/tests-*.xml' EXIT

    echo "== Installing the current builder script"
    "$ucom" add builder --force "$project"

    echo "== Batch build ($target)"
    "$ucom" build "$target" "$project" -q -o "$out/build" -l "$out/build.log"

    echo "== Batch build with a failing pre-build hook (must fail because of the hook)"
    if output=$("$ucom" build "$target" "$project" -q -o "$out/hook" -l "$out/hook.log" \
        --build-args fail 2>&1); then
        echo "Expected the build to fail" >&2; exit 1
    fi
    if ! grep -q "asked to fail" <<<"$output"; then
        echo "The build failed, but not because of the hook:" >&2; echo "$output" >&2; exit 1
    fi

    echo "== EditMode tests without the Failing category"
    "$ucom" test editmode "$project" -r errors --categories '!Failing'

    echo "== EditMode tests in the Failing category (must exit with 2)"
    status=0
    "$ucom" test editmode "$project" -r errors --categories Failing || status=$?
    if [ "$status" -ne 2 ]; then
        echo "Expected exit code 2, got $status" >&2; exit 1
    fi

    echo "== PlayMode tests"
    "$ucom" test playmode "$project" -r errors

    echo "== All real Unity checks passed"

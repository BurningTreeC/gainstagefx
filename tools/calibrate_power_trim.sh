#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

new_table="$(mktemp)"
old_table="$(mktemp)"
verify_table="$(mktemp)"
cleanup() {
    rm -f "$new_table" "$old_table" "$verify_table"
}
trap cleanup EXIT

cp src/power_trim.rs "$old_table"

echo "Generating power-stage calibration into a temporary file..." >&2
cargo run --release --example powertrim > "$new_table"

# Refuse to replace the source with a partial/error file. The generator writes
# diagnostics to stderr, so stdout should contain only valid Rust source.
grep -q '^pub const POWER_TRIM_DB:' "$new_table"
grep -q '^];$' "$new_table"

cp "$new_table" src/power_trim.rs

rollback() {
    echo "Power-trim calibration failed; restoring the previous table." >&2
    cp "$old_table" src/power_trim.rs
}

if ! cargo test --release --test power_trim -- --nocapture; then
    rollback
    exit 1
fi

echo "Verifying that a second calibration pass is stable..." >&2
if ! cargo run --release --example powertrim > "$verify_table"; then
    rollback
    exit 1
fi

if ! diff -u src/power_trim.rs "$verify_table"; then
    echo "Power-trim generation is not stable across two consecutive passes." >&2
    rollback
    exit 1
fi

echo "Power-trim calibration succeeded and is stable." >&2

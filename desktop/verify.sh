#!/usr/bin/env bash
# Everything about the desktop app that can be verified on a macOS development
# machine, in one place.
#
# Honest about what each step proves:
#   - the host test suite RUNS
#   - the Windows and Linux platform code is only TYPE-CHECKED, never executed
set -euo pipefail

export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
red=$'\033[31m'; green=$'\033[32m'; bold=$'\033[1m'; off=$'\033[0m'

step() { printf '\n%s==> %s%s\n' "$bold" "$1" "$off"; }
ok()   { printf '%s    ok%s %s\n' "$green" "$off" "$1"; }

step "Host tests (macOS) — these actually run"
cd "$here/src-tauri"
cargo test --lib
ok "host suite"

step "Web view — pure logic, and the seams either side of it"
# There is no bundler and no browser here, so this covers what can be checked
# without one: the decisions in logic.js, and the joins between main.js, the HTML,
# and the Rust command list. It does NOT render anything or click anything.
cd "$here/src"
node --test
ok "web view logic and wiring"

step "Clippy, all targets, warnings as errors"
cd "$here/src-tauri"
cargo clippy --all-targets -- -D warnings
ok "clippy clean"

step "macOS keep-awake against the OS itself"
# A unit test can only check our own bookkeeping. This asks pmset whether the
# assertion really appeared and really went away.
probe_output="$(cargo run --quiet --example assertion_probe)"
printf '%s\n' "$probe_output"
if ! grep -q ASSERTION_PROBE_OK <<<"$probe_output"; then
    printf '%s    FAILED%s the IOKit assertion was not visible to pmset\n' "$red" "$off"
    exit 1
fi
ok "IOKit assertion observed by pmset"

step "Windows and Linux platform code — TYPE-CHECK ONLY, not a build, not a run"
# The main crate cannot be cross-compiled here: bundled SQLite needs a Windows C
# toolchain and Tauri needs Linux GTK headers. platform-check pulls in the real
# per-OS sources with #[path] and depends on nothing needing a C compiler, so the
# code that would otherwise never be compiled at all is at least type-checked.
cd "$here/platform-check"
for target in aarch64-pc-windows-msvc aarch64-unknown-linux-gnu; do
    cargo check --target "$target" --all-targets
    ok "$target type-checks"
done

step "Shared test vectors — both implementations against the same cases"
cd "$here/src-tauri"
cargo test --test shared_vectors
ok "rust matches the shared vectors"

printf '\n%s%sVerified.%s Host suite ran on macOS. Windows and Linux platform code is\n' "$bold" "$green" "$off"
printf 'type-checked only — runtime behaviour on those two platforms is UNVERIFIED.\n'

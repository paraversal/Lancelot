#!/usr/bin/env bash
# Runs the same checks as CI (core and ui jobs). Usage: scripts/check.sh
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

if command -v pnpm >/dev/null; then
	pnpm() { command pnpm "$@"; }
else
	version=$(node -p "require('./app/package.json').packageManager.split('@')[1]")
	pnpm() { npx --yes "pnpm@$version" "$@"; }
fi

step "UI: install, tokens, lint, build"
(
	cd app
	pnpm install --frozen-lockfile
	pnpm tokens:check
	pnpm lint
	# The Tauri crate embeds the built UI, so build it before the Rust checks.
	pnpm build
)

# cargo test also regenerates the TypeScript types in app/src/lib/api/generated.
step "Rust: fmt, clippy, tests"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

step "UI: types, tests"
(
	cd app
	pnpm check
	pnpm test
)

step "All checks passed"

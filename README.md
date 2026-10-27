# Lancelot

A local desktop app for settling volunteer-run events: receipts, advances, reimbursements, tickets and drinks, in one file per event.

- [Requirements](docs/requirements.md)
- [Roadmap](docs/roadmap.md)
- [Technical design](docs/technical-design.md)
- [Design notes](docs/design/README.md)

## Development

Requirements: Rust (stable, ≥ 1.90), Node 24, pnpm 12 (the exact version is pinned in `app/package.json`), and Xcode command line tools on macOS. Without a global pnpm, prefix the commands with `npx pnpm@12.15.0` instead of `pnpm`.

```bash
cd app && pnpm install
```

```bash
cd app && pnpm tauri dev
```

Checks, as run in CI:

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

```bash
cd app && pnpm tokens:check && pnpm lint && pnpm check && pnpm test
```

`app/src/lib/styles/tokens.css` is copied from `docs/design/tokens.css` with `pnpm tokens`. Do not edit the copy.

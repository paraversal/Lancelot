# Lancelot

A local desktop app for settling volunteer-run events: receipts, advances, reimbursements, tickets and drinks, in one file per event.

- [Requirements](docs/requirements.md)
- [Roadmap](docs/roadmap.md)
- [Technical design](docs/technical-design.md)
- [Design notes](docs/design/README.md)

## Development

Requirements: Rust (stable, ≥ 1.90), Node 24 with npm ≥ 11.4, and Xcode command line tools on macOS. npm 11.3 fails to install the dependencies, so update it or run npm through `npx npm@11`.

```bash
cd app && npm install
```

```bash
cd app && npm run tauri dev
```

Checks, as run in CI:

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

```bash
cd app && npm run tokens:check && npm run lint && npm run check && npm test
```

`app/src/lib/styles/tokens.css` is copied from `docs/design/tokens.css` with `npm run tokens`. Do not edit the copy.

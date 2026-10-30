# Lancelot

A local desktop app for settling volunteer-run events: receipts, advances, reimbursements, tickets and drinks, in one file per event.

- [Requirements](docs/requirements.md)
- [Roadmap](docs/roadmap.md)
- [Future](docs/FUTURE.md)
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

To look at the screens in a plain browser, against the real core and throwaway event files, run these side by side and open <http://localhost:5173> ([details](docs/technical-design.md#5-ipc-contract)):

```bash
cargo run -p lancelot-bridge
```

```bash
cd app && pnpm dev
```

Run all checks, the same ones CI runs on every push:

```bash
scripts/check.sh
```

Full app builds for macOS, Windows and Linux run weekly and on demand (`.github/workflows/build.yml`).

`app/src/lib/styles/tokens.css` is copied from `docs/design/tokens.css` with `pnpm tokens`. Do not edit the copy.

# Computer Harness

**Implementation status: Milestone 1 scaffolding in progress.**

Computer Harness is a persistent, federated, cross-platform computer-agent runtime with **Fairy** as its adaptive companion and coordinator. The product is CLI-first; a dashboard is deferred until the CLI/runtime is dependable.

## Current source slice

The `build/foundation-m1` branch currently contains:
- Native Rust CLI entry point (`fairy doctor`, `fairy status`, `fairy run`).
- Per-user application data/config directory initialization.
- Machine-readable JSON status and diagnostics that explicitly avoid claiming authentication or platform-control support.
- Foreground supervisor shell with clean Ctrl+C handling.
- Windows/Linux GitHub Actions checks for formatting, Clippy, tests and release build.
- An implementation status ledger that records what is and is not implemented.

This is scaffolding, not the completed persistent daemon or Fairy agent.

## Planning and tracking

- [Planning specification](docs/COMPUTER_HARNESS_PLANNING_SPEC.md)
- [Build tracking checklist](docs/BUILD_TRACKING_CHECKLIST.md)
- [Implementation status](docs/IMPLEMENTATION_STATUS.md)

## Build from source

Install stable Rust with Cargo, then run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Run:

```sh
cargo run -- doctor
cargo run -- status
cargo run -- run
```

## Non-claims

The current source does not yet provide a persistent background daemon, task store, Qoder integration, model routing, permission gateway, tool registry, browser/GUI/computer control, memory, skills, federation, Android support, Gaming Mode or self-improvement. These remain required roadmap work. Hosted CI and owner-device tests must pass before the first slice is accepted.

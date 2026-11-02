# Computer Harness

**Implementation status: early core prototype — not release-ready.**

Computer Harness is a persistent, federated, cross-platform computer-agent runtime with **Fairy** as its adaptive companion and coordinator. The product is CLI-first; a dashboard is deferred until the runtime is dependable.

## Implemented in this branch

- Rust CLI commands: `fairy doctor`, `fairy status`, `fairy serve`, `fairy daemon status`, `fairy daemon shutdown`, `fairy run`.
- Per-user data/config directory initialization.
- SQLite database with WAL mode, schema version marker, durable task records and event log.
- Task commands: `fairy task create <title> <objective>`, `fairy task list`, `fairy task show <id>`, `fairy task cancel <id>`.
- Local stop-request marker via `fairy stop-all` (does not yet terminate workers).\n- Foreground loopback control plane (`fairy serve`) with per-user token authentication, bounded JSON requests, and task/status/shutdown RPC.\n- Persisted exact-name capability grants, action-scoped approval requests, an audit event trail, and a policy evaluator; paid inference/financial actions are denied by the default zero-spend rule.
- Windows/Linux CI workflow for formatting, Clippy, tests and release build.
- Explicit architecture contracts and an implementation status ledger.

The task store persists objectives; it does **not** yet execute them. The `run` command remains a foreground supervisor shell, not an installed background service.

## Build

Install stable Rust and Cargo:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

## Try the current CLI

```sh
cargo run -- doctor
cargo run -- status
cargo run -- task create "Example" "Describe the objective"
cargo run -- task list
cargo run -- serve
# In another terminal:
cargo run -- daemon status
cargo run -- policy grant filesystem.read "Owner approved read-only access"
cargo run -- policy check filesystem.read filesystem.read --effect read-only
cargo run -- approval request filesystem.delete "Remove generated temporary output"
cargo run -- approval list
```

## Planning and evidence

- [Planning specification](docs/COMPUTER_HARNESS_PLANNING_SPEC.md)
- [Build tracking checklist](docs/BUILD_TRACKING_CHECKLIST.md)
- [Architecture contracts](docs/ARCHITECTURE_CONTRACTS.md)
- [Implementation status](docs/IMPLEMENTATION_STATUS.md)

## Not implemented

No auto-starting or detached OS service; the current control plane must be started in a foreground terminal. No task scheduler/worker execution; provider/model routing; Qoder integration; tool-execution permission gateway (the current policy check is advisory and no tools execute); tool registry/execution; browser/GUI/computer control; durable memory/skills; secure device pairing/federation; Linux or Android platform adapters; vision/research pipeline; Gaming Mode/resource governor; notifications; self-improvement; dashboard; installer/updater. These are required project scope, not optional claims.

No hosted model is assumed to provide unlimited zero-cost inference. Paid fallback is prohibited unless explicitly authorized. Windows/Linux CI and owner-device tests must pass before this prototype is accepted.

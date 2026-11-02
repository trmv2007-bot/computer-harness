# Computer Harness

**Implementation status: early prototype — not release-ready.**

Computer Harness is a persistent, federated, cross-platform computer-agent runtime with **Fairy** as its adaptive companion and coordinator. The product is CLI-first; a dashboard is deferred until the runtime is dependable.

## Implemented in this branch

- Rust CLI with diagnostics, local status, control-plane, tasks, workspace, capabilities, approvals, registered tools and optional agent commands.
- SQLite WAL store (schema v3), durable task records, lifecycle transitions, event log, capability grants, action-scoped approvals and workspace settings.
- Foreground loopback control plane (`fairy serve`) with per-user token authentication, bounded requests/responses and task/tool RPC.
- Bounded IPC connection admission, request timeouts and Unix token-file permissions.
- Central policy evaluator: exact capability grants required; destructive/admin/external operations require scoped approval; paid and financial actions are denied by the default zero-spend rule.
- Initial read-only workspace tools: `filesystem.list` and `filesystem.read_text`. Both require `filesystem.read`, reject workspace path escapes, enforce input/output bounds and append audit events.
- Optional Qoder Agent SDK worker (`agent/`), pinned dependencies, custom MCP tools routed through Fairy's IPC gateway, and CLI setup/ask commands. It uses the local signed-in Qoder CLI session; it does not assume unlimited/free inference.
- Automated CI: Rust formatting, Clippy, tests and release builds on Windows/Ubuntu; Node worker syntax/manifest validation.

The current agent can inspect text files and list a configured workspace. It cannot edit files, run shell commands, control the desktop/browser, or perform arbitrary computer actions.

## Build

Install stable Rust/Cargo. For Qoder integration, install Node.js 18+ and sign in with the official Qoder CLI.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

## Try the current CLI

```sh
cargo run -- doctor
cargo run -- workspace set "C:/path/to/your/project"
cargo run -- policy grant filesystem.read "Owner approved workspace read access"
cargo run -- tool list
cargo run -- agent setup

# Terminal 1:
cargo run -- serve

# Terminal 2:
cargo run -- agent ask "Inspect this workspace and summarize its structure."
```

The control plane must currently remain running in a foreground terminal. The optional Qoder worker has not yet been verified against the owner's Qoder account or PC.

## Planning and evidence

- [Planning specification](docs/COMPUTER_HARNESS_PLANNING_SPEC.md)
- [Build tracking checklist](docs/BUILD_TRACKING_CHECKLIST.md)
- [Architecture contracts](docs/ARCHITECTURE_CONTRACTS.md)
- [Implementation status](docs/IMPLEMENTATION_STATUS.md)
- [Qoder worker setup and limits](agent/README.md)

## Not implemented

OS-managed background service/auto-start; reconnect recovery; task scheduler/worker execution; multi-task concurrency, leases and checkpoints; provider/model routing and quota-aware pause/resume; Qoder account authentication/usage verification on owner device; write-capable tools; process sandboxing and full secret redaction; Windows/Linux desktop and browser control; Android adapter; vision/research pipeline; durable hybrid memory; skill engine; secure federation/pairing; notifications; Gaming Mode/resource governor; proactive monitoring; self-improvement; dashboard; installer/updater.

No hosted inference service is assumed to be unlimited or zero-cost. Paid fallback is disabled. Workspace checks use canonical paths but are not an OS-enforced sandbox and are not yet hardened against path races.

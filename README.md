# Computer Harness

**Implementation status: early prototype — not release-ready.**

Computer Harness is a persistent, federated, cross-platform computer-agent runtime with **Fairy** as its adaptive companion and coordinator. The product is CLI-first; a dashboard is deferred until the runtime is dependable.

## Implemented in this branch

- Rust CLI with diagnostics, local status, control-plane, tasks, workspace, capabilities, approvals, registered tools and optional agent commands.
- SQLite WAL store (schema v4), durable tasks and execution attempts, event log, capability grants, action-scoped approvals and workspace settings, durable task records, lifecycle transitions, event log, capability grants, action-scoped approvals and workspace settings.
- Foreground loopback control plane (`fairy serve`) with per-user token authentication, bounded requests/responses and task/tool RPC.
- Explicit user-level startup service commands: Windows Task Scheduler at logon and Linux systemd user service with restart-on-failure. Installation is opt-in; it is never enabled implicitly.
- Bounded IPC connection admission, request timeouts, Unix token-file permissions and a client response reader sized to the 8 MiB server limit.
- Task create/state/cancel and task execution lifecycle changes are committed atomically with their audit events; execution results and usage snapshots persist across CLI sessions.
- Control-plane startup detects task-run attempts left in `running` after an unexpected process exit, atomically marks them failed, records recovery events and requires an explicit retry.
- Central policy evaluator: exact capability grants required; destructive/admin/external operations require scoped approval; paid and financial actions are denied by the default zero-spend rule.
- Initial read-only workspace tools: `filesystem.list` and `filesystem.read_text`. Both require `filesystem.read`, use cap-std directory handles to constrain access to the configured workspace, reject parent/absolute paths and external symlink escapes, enforce input/output bounds, block common credential paths and append audit events.
- Built-in specialist team: Fairy Coordinator, Engineer, Researcher, Operator, Auditor and Archivist. Each profile loads integrated skill playbooks, role-specific operating rules and an explicit current capability boundary into the Qoder system prompt. Task inspections use the Auditor profile by default.
- Optional Qoder Agent SDK worker (`agent/`), pinned dependencies, custom MCP tools routed through Fairy's IPC gateway, strict success/error result propagation, usage/quota reporting, and CLI setup/ask commands. It uses the local signed-in Qoder CLI session; it does not assume unlimited/free inference.
- Automated CI: Rust formatting, Clippy, tests and release builds on Windows/Ubuntu; Node worker syntax/manifest validation.

The current agent can inspect non-sensitive text files and list a configured workspace. Queued tasks can be inspected with a one-shot read-only Qoder run; the output and usage are persisted, then the task pauses for owner review. It cannot edit files, run shell commands, control the desktop/browser, or perform arbitrary computer actions.

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
cargo run -- team list
cargo run -- team show engineer
cargo run -- agent setup
cargo run -- agent ask --role engineer "Review the configured workspace architecture."
cargo run -- task create "Inspect project" "Review the project structure and identify risks"

# Either run in the foreground:
cargo run -- serve

# Or install an opt-in user service (Windows/Linux):
cargo run -- service install
cargo run -- service status
cargo run -- agent ask "Inspect this workspace and summarize its structure."
cargo run -- task inspect <TASK_ID>
cargo run -- task result <TASK_ID>
# Review the persisted result before accepting it:
cargo run -- task accept <TASK_ID>

# To stop or remove the service:
cargo run -- service stop
cargo run -- service uninstall
```

The service manager is implemented for Windows Task Scheduler and Linux systemd user services; the actual installation/startup lifecycle still needs owner-device validation. The optional Qoder worker has not yet been verified against the owner's Qoder account or PC.

## Planning and evidence

- [Planning specification](docs/COMPUTER_HARNESS_PLANNING_SPEC.md)
- [Build tracking checklist](docs/BUILD_TRACKING_CHECKLIST.md)
- [Architecture contracts](docs/ARCHITECTURE_CONTRACTS.md)
- [Implementation status](docs/IMPLEMENTATION_STATUS.md)
- [Qoder worker setup and limits](agent/README.md)

## Not implemented

automatic task scheduling; multi-task concurrency, leases and checkpoints; provider/model routing and quota-aware pause/resume; Qoder account authentication/usage verification on owner device; write-capable tools; process sandboxing and comprehensive secret redaction; Windows/Linux desktop and browser control; Android adapter; vision/research pipeline; durable hybrid memory; versioned/learned skill engine and skill validation; secure federation/pairing; notifications; Gaming Mode/resource governor; proactive monitoring; self-improvement; dashboard; installer/updater.

No hosted inference service is assumed to be unlimited or zero-cost. Paid fallback is disabled. Workspace filesystem access is rooted in cap-std directory capabilities; common credential paths are denied. This does not sandbox the entire Fairy process or protect against an already-compromised same-user process.

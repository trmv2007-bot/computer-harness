# Implementation Status

Updated: 2026-11-02

## Overall state

**NOT COMPLETE — early prototype.** The product is not release-ready. The planning specification is the source of truth; this ledger records implemented and evidenced work only.

## Implemented source (CI evidence varies by commit)

- Rust CLI: diagnostics, status, foreground control-plane, daemon status/shutdown, task CRUD/state operations, workspace configuration, capability/approval administration, tool discovery/execution and optional Qoder agent setup/ask.
- Per-user data/config directory initialization.
- SQLite WAL database at schema version 4: durable tasks and agent attempts, event log, capability grants, action-scoped approvals with one-time consumption, and application settings.
- Schema v1-to-v4 upgrade regression test preserving existing tasks and creating task-run storage.
- Atomic capability/approval mutations and task lifecycle mutations with audit events; regression tests verify rollback when audit insertion fails.
- Foreground IPv4 loopback control plane with a random per-user token, Unix token mode 0600, 64 KiB request limit, 8 MiB response limit enforced by both server and client, 8 concurrent clients, 5-second request read deadline and task/tool RPC.
- Task CLI uses the control plane when reachable and otherwise falls back to local SQLite.
- Central side-effect policy: exact capability grants required; destructive/admin/external actions require an action-scoped approval; paid inference and financial effects are denied by default.
- Typed tool registry with filesystem.list and filesystem.read_text. Workspace-relative paths are validated and accessed through cap-std directory handles; parent traversal and external symlink escapes are denied; common credential paths are blocked; inputs/listings/text reads are bounded; unknown JSON properties are rejected; denied/started/completed/failed calls are audited.
- Built-in specialist team profiles: Fairy Coordinator, Engineer, Researcher, Operator, Auditor and Archivist. Nine integrated prompt-based skill playbooks are selected per role and passed into the Qoder worker as structured profile context. Each profile declares its mission, operating rules, available tool scope and limitations. Task inspections use Auditor by default; CLI users can select a role explicitly.
- Optional Qoder TypeScript SDK worker and Zod dependency pinned in agent/package.json. It reuses the local Qoder CLI sign-in via qodercliAuth(), exposes only Fairy MCP read tools, restricts the visible tool set, propagates explicit SDK success/error subtypes and error codes, and reports available account quota/session usage.
- CLI launches the worker through a child Node process using agent setup and agent ask. Tasks can be inspected through a user-triggered read-only agent attempt; the attempt/result/usage are persisted, and successful provider completion leaves the task paused for owner review. Only explicit owner acceptance marks it complete.
- Opt-in user service registration: Windows Task Scheduler at user logon and Linux systemd user unit with restart-on-failure; service status/start/stop/uninstall commands are exposed. No service is installed automatically.
- CI includes Windows/Ubuntu Rust fmt, Clippy, tests, release builds, plus Node syntax and manifest checks.

## In progress / not verified

- Previous CI run 41400542922 passed the Windows and Ubuntu Rust fmt, Clippy, tests and release-build stages, plus Node syntax/manifest validation, for an earlier commit.
- The specialist-team commits are newer than that run and require a fresh complete green CI run before this slice is marked verified.
- Qoder npm installation, local authentication, actual model request, quota response and end-to-end tool call have not been exercised using the owner's signed-in Qoder account.
- Windows owner-device and iQOO Android validation have not been run.

## Not implemented

Owner-device validation of service install/start/stop/uninstall, Windows service crash recovery, systemd user-manager integration; interrupted task-run recovery; automatic task scheduling/worker queues, concurrency, checkpoints and leases; task leases, retries, checkpoints, dependency DAG and independent completion verification; separate model-provider and agent-engine adapter contracts in code; capability-aware model routing and quota/cost enforcement; provider pause/resume; write-capable tools; OS-level sandboxing; comprehensive secret redaction; reliable stop-all process cancellation; GUI/browser/vision/desktop control; Linux/Android OS adapters; hybrid memory and sync; versioned, user-learned and replay-tested skill packages (the current specialist playbooks are built-in prompt instructions, not persistent executable skills); secure federation/pairing/relay; notifications; Gaming Mode/resource governor; proactive event monitoring; self-development and rollback; dashboard; installer/updater.

## Verification evidence and limitations

- Rust/Cargo are not installed in the authoring environment, so local compilation/tests were not run.
- CI run 41400537374 for commit 41535003f409c12bad2138825ce2a31712a64f7c: Node validation passed; Ubuntu formatting, Clippy, tests and release build passed; Windows formatting and Clippy passed and its tests/build were still in progress at last inspection.
- Commits after the last green run need a final CI run on current HEAD before this slice can be marked verified. The new specialist profile Rust tests and Qoder prompt integration have not yet been exercised against the owner's Qoder account.
- Windows token file protection currently depends on inherited per-user AppData ACLs; explicit ACL hardening and manual verification remain open.
- Workspace file access uses cap-std directory handles, which constrain relative reads and reject escapes through parent traversal/external symlinks. This does not sandbox all process capabilities or defend against a compromised same-user process; common secret path blocks are conservative but not comprehensive.
- Service registration code has CI coverage only; it has not yet been installed or exercised on the owner's Windows PC or a Linux desktop.
- Qoder local authentication/provider behavior has not been tested; no free/unlimited quota is claimed.

## Status rules

A feature is complete only after implementation, acceptance criteria, relevant automated/manual tests and evidence are recorded. Do not infer platform support from shared Rust builds. Do not infer provider quotas or unlimited access from UI labels.

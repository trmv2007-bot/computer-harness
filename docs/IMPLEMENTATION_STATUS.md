# Implementation Status

Updated: 2026-11-02

## Overall state

**NOT COMPLETE — early core prototype.** The repository is not release-ready. The planning specification remains the source of truth; this ledger records only implemented and evidenced work.

## Implemented source (CI-tested at the previous commit)

- Rust CLI: doctor, status, foreground run shell, task and stop-all commands.
- Per-user data/config directory setup.
- SQLite database initialized in WAL mode with schema version marker.
- Durable task create/list/show/cancel operations and validated state transitions.
- Minimal task event log.
- Stop-all request marker (records a request only; does not stop running work).
- Architecture contracts for task, provider, tool, safety, node, memory, skill and resource-governor boundaries.
- Windows/Linux CI workflow; latest observed run passed on both runners for commit `95994a8fefc449b1b21389c989d980460a22f4d3` (format, Clippy, unit tests and release build).

## New control-plane work (awaiting CI on current branch head)

- Foreground local control-plane server bound to IPv4 loopback only.
- Per-user random IPC token; Unix token file permissions are set to owner read/write.
- Bounded (64 KiB) single-line JSON request/response protocol.
- RPC actions: ping, list tasks, get task, create task, cancel task and shutdown.
- Database operations are dispatched to Tokio blocking workers rather than run on the async accept loop.
- The server is manually started with `fairy serve`; it is not installed, detached, or auto-started by the operating system.
- Windows token ACLs currently inherit the user's profile-directory ACL; a dedicated ACL hardening implementation is still required.

## Not implemented

OS-managed background service/auto-start and reconnecting CLI client; task scheduling and worker execution; DAG/dependencies, leases and checkpoint recovery; Qoder/provider adapters; model routing/quota/cost enforcement; tool registry and permission gateway; approval UI; audit/secret redaction; actual process cancellation; Windows/Linux/Android computer-control adapters; browser/GUI/vision/research; memory and synchronization; skill engine; secure pairing/federation; notifications; Gaming Mode/resource governor; self-development; dashboard; installer/updater.

## Verification evidence

- Rust/Cargo are not installed in the authoring environment; local compilation and tests have not been run.
- Previous hosted CI run for commit `95994a8fefc449b1b21389c989d980460a22f4d3` passed formatting, Clippy, unit tests and release builds on Windows and Ubuntu.
- CI for the new IPC commit is pending; no claim is made that the IPC layer compiles until the run is observed.
- Windows owner-device and iQOO Android validation have not been run.
- No Qoder authentication or provider request has been made from this branch.

## Status rules

A feature is complete only after its implementation, acceptance criteria, relevant automated/manual tests and evidence are recorded. Do not infer platform support from a shared Rust build. Do not infer provider quotas or unlimited access from UI labels.

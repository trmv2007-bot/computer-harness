# Implementation Status

Updated: 2026-11-02

## Overall state

**NOT COMPLETE — early core prototype.** The repository is not release-ready. The planning specification remains the source of truth; this ledger records only implemented and evidenced work.

## Implemented source

- Rust CLI: doctor, status, foreground control-plane server, daemon status/shutdown, task management, policy and approval commands.
- Per-user data/config directory setup.
- SQLite WAL database with additive schema version 2, durable tasks and event records.
- Task create/list/show/cancel and validated state transitions.
- Foreground loopback control plane with random per-user token and a bounded JSON protocol.
- Task CLI commands use the control plane when reachable and fall back to local SQLite if it is not.
- RPC: ping, list/get/create/cancel/set-state task, shutdown.
- Explicit exact-name capability grants; wildcard capability names are rejected.
- Approval requests are bound to one exact action; approval resolution is audited.
- One-time approval consumption is stored atomically in the database for the future tool executor. It is not exposed as a standalone command because consuming approval without executing the action would be misleading.
- Policy evaluator distinguishes read-only, reversible write, destructive, administrator, external communication, financial and paid-inference effects.
- Financial and paid-inference actions are hard-denied by the current zero-spend policy.
- Windows/Linux CI workflow for formatting, Clippy, tests and release builds.

## In progress / not verified

- Current policy/approval source changes are awaiting a fresh CI result. The last fully passing cross-platform CI is run `41398076332` at commit `f06b47a6cf131178dfe00e502e50a8dd54ac01a9`, before the policy/approval implementation.

## Not implemented

OS-managed background service installation/auto-start; reconnect/recovery behavior; task scheduler and worker execution; DAG/dependencies, leases and checkpoint recovery; Qoder/provider adapters; model routing/quota/cost metadata and provider pause/resume; actual tool registry/execution and enforcement gateway; interactive approval UI; process sandboxing and secret redaction; operational emergency cancellation; Windows/Linux/Android computer-control adapters; browser/GUI/vision/research; durable memory and sync; skill engine; secure device pairing/federation; notifications; Gaming Mode/resource governor; proactive event monitors; self-development; dashboard; installer/updater.

## Verification evidence and limitations

- Rust/Cargo are not installed in the authoring environment; local compilation and tests have not been run.
- CI run `41398076332` passed Windows and Ubuntu formatting, Clippy, all tests and release builds for the control-plane layer.
- CI for the subsequent policy/approval changes is pending or being repaired; do not treat those changes as verified until a complete green run is recorded.
- Windows owner-device and iQOO Android validation have not been run.
- No Qoder authentication or provider request has been made from this branch.
- Windows IPC token protection currently relies on the inherited ACL of the per-user application-data directory. Dedicated ACL verification is still required.

## Status rules

A feature is complete only after its implementation, acceptance criteria, relevant automated/manual tests and evidence are recorded. Do not infer platform support from a shared Rust build. Do not infer provider quotas or unlimited access from UI labels.

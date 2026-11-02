# Fairy Runtime Integration — 0.3.0

**Status:** Initial implementation slice integrated on `feat/fairy-runtime-0.3.0`; review required; not merged or deployed.  
**Authorization:** `START THE BUILD` received from the owner on 2026-11-02.  
**Prototype artifact:** [Fairy Harness 0.3.0](../runtime/fairy/releases/fairy-harness-0.3.0.zip)  
**SHA-256:** `cc77a8756d99a77a4d071ebd0eb9c3eb777057bd7f4267ce26f83ac62b171896`

## 1. What was integrated

The existing Fairy 0.3.0 source distribution is vendored as a versioned release artifact under `runtime/fairy/releases/`. It is kept separate from the Computer Harness planning document and does not replace the latter's shared-core architecture.

The archive contains:
- Python CLI and persistent local daemon.
- SQLite WAL event/fact storage, FTS5 recall, reminders and read-only document indexing.
- Deterministic concept-vector retrieval plus optional ONNX neural embeddings using user-supplied weights.
- Qoder-backed Claude Code adapter client, persistent transcript/session identity and fail-closed budget governor.
- Initiative/task contracts and sequential Research → Delivery → independent Quality pipeline.
- Fairy-managed Git worktree isolation and configured verification commands.
- Specialist agent/team profiles and reusable skills.
- Approval-gated desktop, browser, Android and learned-recipe brokers.
- Optional Windows HUD, F9 hotkey, SAPI voice/dictation/wake hooks.
- CLI, PowerShell and shell scripts, automated tests and verification report.

## 2. Requirements traceability

| Computer Harness requirement | Fairy 0.3.0 contribution | Status |
|---|---|---|
| Persistent runtime/CLI | Resident daemon, lifecycle CLI, loopback API, SQLite memory | Prototype; Linux smoke passed, Windows target pending |
| Provider/engine adapters | Existing adapter HTTP client and Claude Code team launcher | Partial; live Qoder behavior unverified; no independent Agent SDK implementation |
| Safety/tool gateway | Per-action approval tokens, local API key, allowlists, secret filtering | Partial; OS sandboxing and target Windows verification remain |
| Task contracts/orchestration | Initiative contracts; Research → Delivery → Quality stages | Partial; no general DAG/adaptive scheduler |
| Memory/skills/learning | SQLite/FTS5, deterministic concept vectors, optional ONNX, skill/recipe library | Partial; no federated sync/conflict resolution |
| Windows node | PowerShell, Tk HUD, F9, SAPI, desktop brokers | Code present; target PC not tested |
| Linux node | POSIX launcher and local smoke compatibility | Not a native shared-core node; only container smoke tested |
| Android node | Pinned ADB action broker | Not a native Android runtime/node |
| Federation | None in this slice | Deferred |
| Vision/research | Browser-control broker, on-demand screenshot action, research profiles | Partial; no general screenshot/GUI vision model |
| Proactivity/self-development | Marker/AST discovery, initiative pipeline and worktree delivery | Partial; no open-ended discovery or staged self-promotion system |
| Performance/security/release | Bounded paths, approval tests, 94 offline tests, package scan | Partial; actual device resource budgets and full threat-model review remain |

## 3. Architecture decisions deliberately left open

The planning specification recommends evaluating a Rust-first shared core/CLI with thin native adapters. Fairy 0.3.0 is a Python Windows-first prototype. The integration records this as a deliberate prototype deviation, not a permanent language decision.

Still-open design decisions include:
- Final shared-core language and native packaging.
- Platform-neutral IPC/event protocol.
- Persistent task/event-store and migration model beyond the prototype SQLite schema.
- Independent Qoder model-provider and Agent SDK engine contracts.
- Secure device identity, federation and offline synchronization.
- Resource budgets and Gaming Mode measurements.
- Full adaptive concurrency and dependency-DAG design.

Do not mark these choices resolved merely because the prototype uses Python, SQLite or a local HTTP API.

## 4. Reproducible verification evidence

The archive's verification report records:
- `python -m pytest -q`: 94 passed, Linux container, Python 3.13.5.
- `python -m compileall -q src/fairy`: passed.
- Exact ZIP extraction with Unix executable mode preservation.
- Fresh-copy offline install, init, status, daemon start/stop.
- Local API authentication (401 without key; success with local key).
- Fact write and FTS5 recall, due reminder delivery, clean daemon stop.
- 200 concurrent SQLite writes across 8 worker threads.
- Fake-CLI three-team pipeline and explicit agent tool allowlists.
- No live Qoder requests.

The archive verification report is the detailed evidence record. It also enumerates every environment-specific limitation.

## 5. Deployment notes

This PR does not install the runtime on the owner's PC and does not change any existing adapter authentication/configuration.

After review, unpack the release artifact into a local development directory and follow its README. Before enabling cloud:
1. Confirm the existing Qoder adapter/device login and model alias.
2. Verify adapter `/health`, model free status and live usage telemetry.
3. Keep the Fairy cloud kill switch available and free-only mode enabled.
4. Test one low-impact live request and verify transcript/session continuity.

Before enabling autonomy, configure only exact project roots, ensure the source repo is clean, use a disposable Git worktree and leave automatic checks/notifications conservative. The prototype leaves changes uncommitted for human review.

## 6. What this PR does not claim

- It does not implement every Computer Harness milestone.
- It does not establish production-ready Windows, Linux or Android support.
- It does not implement secure federated networking.
- It does not prove live Qoder authentication, quota behavior or Claude Code tool compatibility.
- It does not establish measured CPU/RAM/GPU/battery budgets.
- It does not provide general screenshot/GUI visual reasoning.
- It does not safely self-modify and promote its own production source.
- It does not merge, deploy or configure the user's machine.

The checklist remains authoritative for unfinished acceptance gates.

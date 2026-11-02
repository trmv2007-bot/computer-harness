# Computer Harness — Build Tracking Checklist

This document is the execution companion to [the planning specification](./COMPUTER_HARNESS_PLANNING_SPEC.md).

**Current status: IN PROGRESS — initial Fairy 0.3.0 prototype integrated on `feat/fairy-runtime-0.3.0`; not merged or deployed.**  
**Implementation authorization:** Owner explicitly supplied `START THE BUILD` on 2026-11-02. This authorizes implementation but does not waive safety, cost, platform, testing or approval requirements.

## Current implementation snapshot

| Gate | Status | Evidence / remaining work |
|---|---|---|
| 0 — Specification | Partial | Existing decisions/spec retained. Some architecture choices (Rust core, IPC, federation, resource budgets) remain open. |
| 1 — Persistent CLI core | Partial | Fairy CLI/daemon, local API, SQLite memory, reminders and health/start-stop smoke are in the 0.3.0 bundle. Multi-client recovery, formal migrations and Windows idle-footprint measurements remain. |
| 2 — Intelligence adapters | Partial | Qoder-backed Claude Code adapter client, stable transcript/session identity and fail-closed quota governor are implemented. Live account/auth/quota and independent Qoder provider-vs-Agent-SDK engine behavior remain unverified. |
| 3 — Tool and safety gateway | Partial | Approval-gated desktop, browser, Android and recipe brokers; loopback API auth; explicit agent tool allowlists. Full OS sandboxing and Windows permission-abuse testing remain. |
| 4 — Planning and concurrency | Partial | Initiative contracts, serialized Research → Delivery → Quality pipeline, isolated Git worktrees, checkpoints and configured verification commands. General dependency DAG, adaptive concurrency, broad locks, cancellation and task leases remain. |
| 5 — Memory, skills and learning | Partial | SQLite/FTS5, deterministic concept-vector retrieval, optional user-supplied ONNX embeddings, skills/recipes. Provenance-complete hybrid scopes, encryption, offline sync/conflict resolution and deletion propagation remain. |
| 6 — Windows-first node | Code prototype only | PowerShell scripts, Windows hotkey/HUD/SAPI adapters exist. No target-PC installation or physical Windows acceptance run yet. |
| 7 — Linux and Android nodes | Not started | Android control broker is not a native Android node. Native Linux/Android lifecycle and cross-platform contract tests are outstanding. |
| 8 — Federation | Not started | No secure pairing, remote transport, relay, leases or offline node synchronization. |
| 9 — Vision and research | Partial | Browser control, on-demand screenshot action, research-team profile and source-conscious workflow. No general visual-understanding model or live browser/device validation. |
| 10 — Proactivity and self-development | Partial | Configured-root marker/AST discovery and guarded initiative pipeline. Open-ended discovery and isolated source self-development with canary promotion/rollback are incomplete. |
| 11 — Performance, security and release | Partial | 94 offline tests, compile, clean-archive install/runtime smoke and packaging/privacy audit recorded. Real Windows/Linux/Android performance, Gaming Mode, threat-model review and end-to-end provider/device checks remain. |

## Rules for the build

- [x] Read the planning specification before the first implementation slice.
- [x] Record explicit implementation authorization.
- [ ] Convert every remaining requirement into a tracked issue or task with owner, dependency and acceptance criteria.
- [ ] Resolve open decisions or record explicitly owner-approved defaults.
- [x] Build in small, reviewable milestones.
- [x] Maintain a requirements-to-code-to-test traceability record for the Fairy 0.3.0 slice.
- [ ] Extend traceability to every requirement and supported platform.
- [x] Never mark a feature complete based only on generated code or a model's claim.
- [ ] Test the actual runtime on each supported platform; do not infer Android/Linux support from Windows tests.
- [ ] Complete the Windows-first rollout on the owner's real Windows machine.
- [x] Record exact automated test command, environment, results and limitations for Fairy 0.3.0.
- [ ] Measure idle footprint and host responsiveness on target hardware.
- [ ] Maintain complete migrations, backup and rollback paths for all persistent user data.
- [x] Keep cloud disabled by default and no paid fallback in the Fairy sample configuration.
- [x] Preserve the CLI-first direction; the HUD is a local companion interface, not a dashboard replacement.
- [ ] Benchmark core and optional workers separately on target hardware.
- [ ] Keep always-on core separate from on-demand heavy workers with bounded queues and concurrency.
- [ ] Implement Linux and Android against shared platform-neutral core contracts.

## Milestone gates

### Gate 0 — Specification

- [x] Review existing agreed decisions with the owner.
- [ ] Resolve all architecture interfaces and threat-model questions.
- [ ] Confirm the complete MVP scope without dropping later requirements.
- [ ] Define measurable acceptance criteria and a test matrix for all platforms.

### Gate 1 — Persistent CLI core

- [x] CLI lifecycle and structured command output (Fairy 0.3.0 prototype).
- [x] Persistent daemon lifecycle, health endpoint and clean start/stop smoke (Linux container).
- [x] Local SQLite event/fact memory and FTS5 recall.
- [ ] Verify restart recovery of all task state on Windows.
- [ ] Verify multiple CLI clients observe consistent state.
- [ ] Complete task/event checkpoints, retry reconciliation and configuration migrations.
- [ ] Measure idle CPU wakeups, resident memory, startup time and CLI responsiveness on Windows.
- [ ] Demonstrate that closing CLI sessions does not increase background resource consumption.

### Gate 2 — Intelligence adapters

- [x] Prototype Qoder-backed Claude Code adapter client and fail-closed cloud gate.
- [ ] Separate and independently configure Qoder provider and Qoder Agent SDK engine adapters.
- [ ] Verify official Qoder authentication and account quota behavior on the user's account.
- [ ] Verify shared Qoder quota is never double-counted.
- [ ] Verify capability-aware quality-first routing.
- [x] Hard zero-spend/offline mode in the sample configuration and tests.
- [ ] Verify queue/resume, provider outage and context handoff on live adapter sessions.
- [ ] Verify no paid fallback in every routing path.

### Gate 3 — Tool and safety gateway

- [x] Approval-gated prototype brokers for desktop, browser, Android and learned recipes.
- [x] Local API authentication, secret redaction and audit records in tested paths.
- [x] Explicit tool allowlists for Research, Delivery and read-only Quality stages.
- [ ] Complete OS-level sandbox enforcement and permission-abuse tests on Windows.
- [ ] Verify cancellation, timeouts and side-effect classification across all tools.
- [ ] Verify universal emergency stop works offline and stops active workers.

### Gate 4 — Planning and concurrency

- [x] Task/initiative contracts and sequential Research → Delivery → Quality pipeline.
- [x] Isolated Git worktree for automated Delivery changes.
- [x] Configured verification commands execute as fixed argv arrays without a shell.
- [ ] Implement a general dependency DAG, adaptive concurrency and bounded worker queues.
- [ ] Implement file/application locks, task leases and idempotent retry safeguards.
- [ ] Verify cancellation, partial failure and result aggregation.

### Gate 5 — Memory, skills and learning

- [x] SQLite/FTS5 memory and deterministic local retrieval.
- [x] Optional ONNX embedding adapter requiring user-supplied weights.
- [x] Packaged specialist profiles, team manifests, skills and tested recipe replay.
- [ ] Complete provenance, scope enforcement and user inspection/correction/export/deletion.
- [ ] Implement encrypted offline synchronization and deterministic conflict resolution.
- [ ] Verify context compression/restoration and deletion propagation.

### Gate 6 — Windows-first operating-system node

- [x] Windows-specific PowerShell/HUD/hotkey/SAPI adapters included in the prototype.
- [ ] Install and validate the CLI, daemon, tools, permissions and recovery on the actual Windows PC.
- [ ] Verify F9, orb rendering, speech input/output and voice/wake phrase with target hardware.
- [ ] Validate Gaming Mode and actual resource impact.
- [ ] Record Windows-specific limitations and portability risks.

### Gate 7 — Linux and Android nodes

- [ ] Implement and test the Linux adapter against shared capability contracts.
- [ ] Implement a native Android node with documented lifecycle/background constraints.
- [ ] Verify per-node pairing, permissions and revocation.
- [ ] Run cross-platform contract and interoperability tests.

### Gate 8 — Federation

- [ ] Secure node identity and pairing.
- [ ] LAN transport and authenticated/encrypted remote transport.
- [ ] Approved relay fallback, if selected.
- [ ] Node discovery, task leases and duplicate prevention.
- [ ] Offline work and reconnection reconciliation.

### Gate 9 — Vision and research

- [x] Prototype browser automation with host allowlists and approval gates.
- [x] On-demand screenshot action is available through the desktop broker.
- [ ] Implement visual understanding and outcome verification on supported Windows UI.
- [ ] Validate research citations, dates, uncertainty and source provenance with live sources.
- [ ] Verify no false claims about inaccessible media/private resources.

### Gate 10 — Proactivity and self-development

- [x] Configured-root, read-only marker/AST opportunity discovery prototype.
- [x] Initiative contracts and Research → Delivery → Quality review pipeline.
- [ ] Extend discovery to evidence-backed opportunities beyond current marker/AST heuristics.
- [ ] Implement adaptive notifications and quiet/Gaming Mode behavior.
- [ ] Implement isolated source self-development branches, regression/security gates and staged promotion.
- [ ] Verify rollback and protected safety-supervisor invariants.

### Gate 11 — Performance, security and release readiness

- [x] 94 offline tests passed for Fairy 0.3.0 in Linux/Python 3.13.5.
- [x] Fresh archive extraction, local install, daemon, authenticated API, memory/reminder smoke and clean stop passed.
- [x] Package privacy scan excludes local credentials, personal memory and runtime state.
- [ ] Run Windows, Linux and Android acceptance suites on actual target devices.
- [ ] Measure Gaming Mode and idle/active CPU/RAM/GPU/disk/network budgets.
- [ ] Benchmark bounded concurrency and verify backpressure.
- [ ] Complete threat-model review and permission-abuse testing.
- [ ] Run crash, network-loss, provider-exhaustion and recovery tests on each node.
- [ ] Complete end-to-end requirement traceability review.
- [ ] Obtain owner review before declaring the first release ready.

## Definition of done

A requirement is complete only when:
1. Its implementation exists.
2. Its acceptance criteria are explicit.
3. Relevant automated and manual tests have actually run.
4. Evidence is recorded.
5. Known limitations are stated.
6. Its status is traceable in the specification/checklist.

Never hide a failed, skipped or untested requirement behind a general "done" claim.

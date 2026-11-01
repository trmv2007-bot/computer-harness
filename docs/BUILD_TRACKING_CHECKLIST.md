# Computer Harness — Build Tracking Checklist

This document is the execution companion to [the planning specification](./COMPUTER_HARNESS_PLANNING_SPEC.md).

**Current status: NOT STARTED — planning only.**  
**Implementation authorization phrase:** `START THE BUILD`

## Rules for the eventual build

- [ ] Read the complete planning specification before implementation.
- [ ] Convert every requirement into a tracked issue or task with an owner, dependency and acceptance criteria.
- [ ] Resolve open decisions or record explicit, user-approved defaults.
- [ ] Build in small, reviewable milestones.
- [ ] Keep a requirements-to-code-to-test traceability matrix.
- [ ] Never mark a feature complete based only on generated code or a model's claim.
- [ ] Test the actual runtime on each supported platform; do not infer Android/Linux support from Windows tests.
- [ ] Record exact test commands, environment, results and limitations.
- [ ] Maintain migration, backup and rollback paths for user data and runtime updates.
- [ ] Keep costs at zero unless the owner explicitly authorizes a paid service.
- [ ] Preserve the CLI-first direction; dashboard is deferred.
- [ ] Record scope changes in the planning specification and explain their impact.

## Milestone gates

### Gate 0 — Specification
- [ ] Review all agreed decisions with the owner.
- [ ] Resolve architecture interfaces and threat model.
- [ ] Confirm MVP scope without dropping later requirements.
- [ ] Define measurable acceptance criteria and test matrix.

### Gate 1 — Persistent CLI core
- [ ] CLI lifecycle and structured command output.
- [ ] Persistent background runtime and reconnecting sessions.
- [ ] Durable task/event store, checkpoints and recovery.
- [ ] Diagnostics, logs, configuration and migrations.

### Gate 2 — Intelligence adapters
- [ ] Separate model-provider and agent-engine interfaces.
- [ ] Qoder provider and Agent SDK adapters are independently configurable.
- [ ] Official authentication only; account quota behavior verified.
- [ ] Capability-aware quality-first model routing.
- [ ] Hard zero-spend mode and queue/resume behavior.
- [ ] Provider outage and context handoff tests.

### Gate 3 — Tool and safety gateway
- [ ] Typed tool registry and platform capability declarations.
- [ ] Permission checks, approval workflow and audit trail.
- [ ] Sandboxed execution and secret redaction.
- [ ] Cancellation, timeouts and side-effect classification.
- [ ] Local emergency stop works offline.

### Gate 4 — Planning and concurrency
- [ ] Task contracts and dependency DAG.
- [ ] Adaptive concurrency and bounded work queues.
- [ ] File/application locks and idempotent retry behavior.
- [ ] Worker isolation, partial failure and result aggregation.
- [ ] Independent verification and evidence-based completion.

### Gate 5 — Memory, skills and learning
- [ ] Hybrid memory scopes and provenance.
- [ ] Offline synchronization/conflict resolution.
- [ ] Skill packaging, versioning and validation.
- [ ] Context compression/restoration tests.
- [ ] Inspection, correction, export and deletion behavior.

### Gate 6 — Operating-system nodes
- [ ] Windows adapter and integration tests.
- [ ] Linux adapter and integration tests.
- [ ] Android node with lifecycle/background restrictions documented and tested.
- [ ] Per-node permissions, pairing and revocation.

### Gate 7 — Federation
- [ ] LAN connectivity and secure remote transport.
- [ ] Relay fallback decision and implementation, if approved.
- [ ] Node discovery, task leases and duplicate prevention.
- [ ] Offline work and reconnection reconciliation.

### Gate 8 — Vision and research
- [ ] Browser automation with permission boundaries.
- [ ] Screenshot/GUI understanding and outcome verification.
- [ ] Research source tracking, citation and uncertainty reporting.
- [ ] No false claims about inaccessible media or private resources.

### Gate 9 — Proactivity and self-development
- [ ] High-proactivity event monitoring with bounded overhead.
- [ ] Full project initiative inside configured authority.
- [ ] Adaptive notifications and quiet/Gaming Mode behavior.
- [ ] Isolated self-development branches/workspaces.
- [ ] Regression, security, staged-promotion and rollback gates.
- [ ] Protected safety supervisor and audit controls.

### Gate 10 — Performance, security and release readiness
- [ ] Gaming Mode performance measurements on target hardware.
- [ ] CPU/RAM/GPU/disk/network resource governor tests.
- [ ] Threat-model review and permission abuse tests.
- [ ] Crash, network-loss, provider-exhaustion and recovery tests.
- [ ] End-to-end acceptance suite and requirement traceability review.
- [ ] Document known limitations and deferred requirements.
- [ ] Owner review before declaring the first release ready.

## Definition of done

A requirement is complete only when:
1. Its implementation exists.
2. Its acceptance criteria are explicit.
3. Relevant automated and manual tests have actually run.
4. Evidence is recorded.
5. Known limitations are stated.
6. The requirement's status is traceable in the build checklist.

Never hide a failed, skipped or untested requirement behind a general "done" claim.

# Architecture Contracts (provisional)

This document defines boundaries for implementation. It does not claim the adapters below exist.

## Control plane
The long-lived control plane owns task state, event dispatch, policy checks, worker admission, provider health, checkpoints and audit records. It must not run browser automation, screenshots, indexing, builds or model inference inside its event loop.

## Persistent task contract
A task has an immutable identifier, human-readable title, objective, lifecycle state, timestamps, dependency references, resource budget, permission envelope, checkpoints, evidence and recovery policy. The current SQLite schema implements identity, title, objective, state, timestamps, event log, capability grants, action-scoped approvals and app settings. Task scheduling, worker leases, checkpoints, retry/recovery and durable result/evidence records remain future work. Task create/state/cancel operations and their task events are now transactional.

Allowed state transitions (target contract):
- queued -> running | paused | cancelled | failed
- running -> paused | completed | failed | cancelled
- paused -> queued | cancelled | failed
- failed -> queued (explicit retry only)
- completed and cancelled are terminal

A task must not be marked completed merely because a model returned a success message. Completion requires an independent verifier and recorded evidence.

## Provider contract
A provider adapter must expose: provider identity, official authentication status, model capability metadata, quota/rate-limit state when available, request/cancel operations, streaming support, usage metadata and normalized errors. It must declare whether requests can incur cost. The global zero-spend policy denies paid routes before dispatch. Provider quota exhaustion pauses or queues work; it must not trigger paid fallback.

The Qoder model/provider adapter and Qoder Agent SDK/engine adapter are separate integration points. They may share the same account entitlement. No independent quota pool may be assumed without evidence. An optional TypeScript Qoder Agent SDK worker now reuses local CLI authentication via qodercliAuth(). It exposes only Fairy-owned read-only MCP tools through authenticated loopback IPC. The separate model-provider adapter, quota-aware routing, and end-to-end account authentication/quota verification remain unimplemented or unverified.

## Tool contract
Every tool registration must declare:
- stable name and version;
- typed input/output schema;
- supported platforms and capability requirements;
- side-effect class (read-only, reversible write, destructive, external communication, financial);
- permission scope and approval requirements;
- timeout, cancellation behavior and resource estimate;
- evidence emitted and verification strategy.

Target rule: all invocations pass through a single permission gateway and append an audit event. Models never receive raw unrestricted shell or filesystem authority by default. Current implementation: typed registry, exact-name capability grants, action-scoped approvals, one-time approval consumption and side-effect evaluator exist. The first two tools are workspace-bounded read-only filesystem operations. Canonical path checks are not an OS sandbox and remain vulnerable to path races.

## Safety contract
Sensitive operations require an explicit capability grant and, where configured, an interactive approval. Destructive, administrator and external-communication effects require approval; financial and paid-inference actions are hard-denied by the current zero-spend policy. Approval records are action-scoped and support one-time consumption. Secrets are not yet comprehensively redacted from logs. Stop-all must work locally without network access and prevent new work from being admitted. A stop marker alone does not terminate already-running workers; a supervisor with cancellation tokens and process ownership is required before this feature can be called operational. The control plane can now be registered explicitly as a Windows logon task or Linux systemd user service; those OS lifecycle paths remain device-unverified.

## Node contract
Each node has a unique identity, local durable store, capability manifest and independently revocable credentials. Remote tasks use leases and idempotency keys. Reconnection must reconcile event histories without duplicating side effects. Transport must authenticate and encrypt peers; relay is optional and must not become a plaintext fallback.

## Memory and skills
Memory records require scope, provenance, timestamp, confidence and deletion/export semantics. Skills are versioned executable workflows with declared tools/permissions, replay tests and a rollback pointer. A learned skill is untrusted until validated.

## Resource governor
The governor uses measured per-device budgets and workload classes. It can defer or suspend optional workers, but cannot terminate user applications or change system-wide power/performance settings without authorization. Local inference remains disabled by default during Gaming Mode.

## Evidence states
Use only: verified, partial, unverified, failed, blocked. Evidence includes exact command, platform, version, result and relevant logs. An unsupported platform is blocked/unverified, never silently marked passed.

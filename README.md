# Computer Harness

**Status: implementation underway.** The owner authorized the build with `START THE BUILD` on 2026-11-02. The first Fairy runtime prototype (0.3.0) is integrated on the review branch `feat/fairy-runtime-0.3.0`; it has not been merged into `main`.

Computer Harness is a persistent, federated, cross-platform computer-agent runtime. **Fairy** is its adaptive companion and primary coordinator. The original planning specification remains the architectural source of truth; this integration is a Windows-first prototype, not a claim that every platform or milestone is complete.

## Project documents

- [Planning specification](docs/COMPUTER_HARNESS_PLANNING_SPEC.md) — agreed product decisions, conceptual architecture, autonomy, providers, memory, self-development, security and acceptance criteria.
- [Build tracking checklist](docs/BUILD_TRACKING_CHECKLIST.md) — milestone gates and current evidence status.
- [Fairy runtime integration record](docs/FAIRY_RUNTIME_INTEGRATION.md) — what 0.3.0 supplies, how it maps to the specification, verification and outstanding work.

## Current implementation

The current prototype is distributed as a versioned source bundle:

- [Fairy Harness 0.3.0 source archive](runtime/fairy/releases/fairy-harness-0.3.0.zip)
- SHA-256: `cc77a8756d99a77a4d071ebd0eb9c3eb777057bd7f4267ce26f83ac62b171896`
- Exact archive size: 166,555 bytes.
- Verification baseline: Linux, Python 3.13.5, 94 offline tests passed; see the integration record for limitations.

The bundle contains a Python CLI/daemon prototype, local SQLite/FTS5 memory, optional Qoder-backed Claude Code adapter client, budget controls, task/initiative workflow, specialist agent/team profiles, approval-gated desktop/browser/Android brokers, voice hooks and a local HUD. Hardware- and provider-dependent capabilities remain unverified until tested on the target Windows machine.

## Architecture and scope discipline

The planning specification recommends evaluating a Rust-first shared core with thin platform adapters. Fairy 0.3.0 is a Python Windows-first vertical slice. That is an implementation bridge to test product behavior; it does **not** silently settle the open language, IPC, provider/engine, federation or resource-budget decisions.

Current release limitations are explicit:

- Windows target hardware and microphone have not been tested here.
- Live Qoder authentication, model generation and Claude Code tool compatibility have not been exercised.
- Linux/Android native nodes and federated networking are not implemented.
- Browser/ADB integration is protocol-tested with mocks, not live devices.
- Adaptive Gaming Mode, full GUI vision, general dependency-DAG parallelism and staged self-development promotion remain incomplete.

See the integration record and checklist before treating any gate as complete.

## Safety and cost rules

- The existing Qoder adapter/login is an optional provider/engine integration; this prototype does not replace it or bundle credentials.
- Cloud use is disabled by default. No paid fallback, credit purchase, provider-limit bypass or automatic quota evasion is allowed.
- Project roots must be explicitly allowlisted. Automated delivery uses a Fairy-managed worktree and leaves changes uncommitted for review.
- Browser, Android, voice and computer actions are disabled or constrained by default and require explicit configuration/approval.
- Do not commit local config, API keys, transcripts, SQLite databases, usage ledgers, screenshots, browser profiles or personal memory.

## Working with the prototype

Extract the archive to a local development directory, then follow its bundled README. Do not unpack the runtime over an existing Computer Harness installation until migration and backup procedures have been reviewed.

The current PR is intentionally not merged. Merge only after reviewing its file list, implementation record and remaining acceptance gates.

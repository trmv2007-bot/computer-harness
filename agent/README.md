# Qoder Agent Worker

This optional worker connects Fairy to the official Qoder Agent SDK.

## Requirements

- Node.js 18 or later.
- A Qoder CLI session already signed in on this Windows/Linux account.
- The Fairy control plane running with `fairy serve`.
- A workspace configured with `fairy workspace set <directory>`.
- The capability `filesystem.read` explicitly granted.

## Install

From this directory:

```sh
npm install
npm run check
```

The package pins `@qoder-ai/qoder-agent-sdk` 1.0.58 and `zod` 4.6.7. SDK installation runs the package's normal setup; it may download the compatible Qoder CLI runtime. Do not put a PAT or other secret in this repository. The worker uses `qodercliAuth()` to reuse the local Qoder sign-in.

## Built-in specialist team

Run `fairy team list` to inspect the available specialists, or `fairy team show engineer` for one profile's mission, integrated skill playbooks, operating rules and tool limits. Select a profile with `fairy agent ask --role engineer "..."`; task inspection currently selects the Auditor profile. These are prompt-specialized roles executed by the same Qoder worker, not separate concurrent processes. Their playbooks refine reasoning but cannot grant tools. All profiles receive the two read-only workspace tools described below. On Windows only, Fairy and Operator can also receive `desktop_list_windows` when the owner explicitly grants `desktop.observe`; the returned visible window titles may contain private information. This is observation only, not clicking or typing. Researcher cannot browse the public web, Operator cannot control applications, and Archivist cannot write persistent memory.

## Security model in this first integration

The worker exposes only two Fairy MCP tools:
- `filesystem_list`
- `filesystem_read_text`
- `desktop_list_windows` (Windows only, Fairy/Operator profiles only, and only with the explicit `desktop.observe` capability)

It disables Qoder's built-in tool set, denies shell/edit/write/agent/web tools, uses `dontAsk` permission mode, and caps a turn at 8 model turns. The MCP handlers call the Rust control plane over authenticated loopback IPC. Rust re-checks the capability grant on every invocation, opens the configured workspace through a cap-std directory capability, rejects parent traversal and external symlink escapes, blocks common credential locations/files, and appends an audit event.

No arbitrary shell, file write, browser, click or keyboard tools are exposed yet. The optional Windows window-list tool is read-only and returns bounded window metadata only. Directory capability access constrains the two read tools; it does not sandbox the entire Fairy/Qoder process. Secret-path blocking is conservative and is not comprehensive secret detection.

Qoder receives the prompt and any file content intentionally read by the tools for remote inference. Local sign-in is used; the worker does not create or store a PAT. SDK quota information is reported when Qoder makes it available, and explicit result error subtypes/error codes are propagated to the CLI. A successful SDK result is stored as an inspection attempt, not treated as completion of the user's task. The task is paused for owner review and must be explicitly accepted. No unlimited or zero-cost entitlement is assumed.

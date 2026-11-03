# Fairy runtime artifact

This folder currently contains integration metadata only; the Fairy 0.3.0 binary source archive has not yet been vendored into Git.

- Separately delivered release: `fairy-harness-0.3.0.zip` (download from the ChatGPT conversation)
- Exact size: 166,555 bytes
- SHA-256: `cc77a8756d99a77a4d071ebd0eb9c3eb777057bd7f4267ce26f83ac62b171896`
- Detailed integration status: [../../docs/FAIRY_RUNTIME_INTEGRATION.md](../../docs/FAIRY_RUNTIME_INTEGRATION.md)

Before claiming the runtime is part of this repository, upload the ZIP with a binary-capable Git workflow and verify the checksum after download.

The archive is intentionally preserved as a release bundle. Extract it into a separate local directory and follow its README. Do not unpack over a live installation or commit local config, memory, logs, API keys or adapter credentials.

The Python implementation is an initial Windows-first prototype, not the final shared-core technology decision. See the Computer Harness planning spec and build checklist.

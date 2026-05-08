# Process Console

Process Console is a macOS desktop tool for finding and cleaning up local server processes started by coding agents.

[中文文档](./README.zh-CN.md)

## Why

Modern coding agents and AI editors often start development servers, MCP servers, test watchers, or helper processes in the background. When an agent session closes, restarts, or loses track of its process tree, those processes can keep running and continue occupying local ports.

Process Console gives you a focused view of those processes so you can answer:

- Which local ports are currently listening?
- Which project does each port probably belong to?
- Was this process likely started from Cursor, Claude Code, opencode, Codex, or OpenClaw?
- Can I terminate the related process group safely from a GUI?

## Features

- macOS process and listening-port scanner.
- Project-grouped process list.
- Agent tab for likely coding-agent-owned processes.
- Other tab for unknown listening processes, so unrelated apps stay out of the main view.
- Built-in attribution rules for Cursor, Claude Code, opencode, Codex, and OpenClaw.
- Confidence badges with evidence for every attribution.
- Masked command-line display for token-like and secret-like arguments.
- Process-group termination with confirmation, using `SIGTERM` first and force-kill fallback when needed.
- Manual refresh plus automatic 5-second refresh.

## Attribution Model

Process Console does not install a background daemon in v1. It scans the current system state when the app is open.

Agent attribution is therefore confidence-based. It combines observable signals such as process ancestry, process group membership, command-line arguments, known app paths, and project working directories. If the original agent has already exited, attribution may be partial.

## Development

Prerequisites:

- macOS
- Node.js 22+
- pnpm
- Rust stable

Install dependencies:

```bash
pnpm install
```

Run in development:

```bash
pnpm dev
```

Run checks:

```bash
pnpm test
pnpm exec tsc --noEmit
pnpm vite build
```

Build the app:

```bash
pnpm build
```

The macOS app bundle is written to:

```text
src-tauri/target/release/bundle/macos/Process Console.app
```

Build a DMG locally:

```bash
pnpm tauri build --bundles dmg
```

## Releases

GitHub Actions builds macOS DMG artifacts and publishes them to GitHub Releases.

Create a release by pushing a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow can also be started manually from the GitHub Actions tab.

## License

MIT


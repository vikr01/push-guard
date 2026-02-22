# push-guard

> Work in progress. See [TODOS.md](./TODOS.md) for the roadmap.

Git push authorization manager for [Claude Code](https://claude.ai/claude-code) hooks.

## Install

```bash
cargo install push-guard
```

Or from source:

```bash
cargo install --path .
```

## What it does

Tracks which git branches Claude created and enforces push authorization rules:

- **Claude-created branches** — pushed to freely, no prompt
- **Protected branches** (`main`, `master`, `trunk`, `develop`) — always blocked, asks for explicit authorization
- **Foreign branches** (exist but Claude didn't create them) — blocked until you grant one-time authorization
- **Force pushes** — always blocked, asks for explicit authorization

## Usage

```
push-guard check --repo <path> --branch <branch> [--force]
push-guard track --repo <path> --branch <branch>
push-guard authorize --repo <path> --branch <branch>
push-guard revoke --repo <path> --branch <branch>
push-guard list [--repo <path>]
```

## Hook setup

See [TODOS.md](./TODOS.md) for the full hook script. Drop it at:

```
~/.claude/hooks/block-main-push.sh
```

## State

Stored at `~/.local/share/push-guard/state.json`. Contains no personal information — only repo paths and branch names.

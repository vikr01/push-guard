# push-guard

- Git push authorization manager for [Claude Code](https://claude.ai/claude-code) hooks
- See [TODOS.md](./TODOS.md) for the full roadmap

## Install

```bash
cargo install push-guard
```

## What it does

- Tracks which git branches Claude created, enforces push authorization rules
  - Claude-created branches — pushed to freely, no prompt
  - Protected branches (`main`, `master`, `trunk`, `develop`) — always blocked, prompts for authorization
  - Foreign branches — blocked until one-time authorization is granted
  - Force pushes — always blocked, prompts for authorization

## Usage

```
push-guard hook
push-guard check   --repo <path> --branch <branch> [--force]
push-guard track   --repo <path> --branch <branch>
push-guard authorize --repo <path> --branch <branch>
push-guard revoke  --repo <path> --branch <branch>
push-guard list  [--repo <path>]
```

## Hook setup

- Add to `~/.claude/settings.json`
  ```json
  {
    "hooks": {
      "PreToolUse": [
        { "matcher": "Bash", "hooks": [{ "type": "command", "command": "/path/to/push-guard hook" }] }
      ]
    }
  }
  ```

## State

- Stored at `~/.local/share/push-guard/state.json`
- Repo paths and branch names only — no personal information

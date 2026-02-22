# push-guard — Roadmap

---

## Phase 0 — Core (complete)

- [x] `check` — exits 0 (allow) or 1 (block); hook converts 1 → exit 2 for Claude Code soft block
- [x] `track` — mark a branch as Claude-created
- [x] `authorize` — grant one-time push authorization to a branch
- [x] `revoke` — revoke a previously granted authorization
- [x] `list` — show all tracked and authorized branches
- [x] State persisted at `~/.local/share/push-guard/state.json`
- [x] Hook script: auto-tracks on `git checkout -b` / `git switch -c`; delegates all push
      decisions to push-guard; falls back gracefully if push-guard not installed

---

## Phase 1 — Robustness

- [ ] Handle `git push origin HEAD:main` (explicit refspec with colon) — currently only
      extracts the last token; refspec destination needs its own parse path
- [ ] Handle `git push` with no remote or branch — look up tracking branch via
      `git rev-parse --abbrev-ref @{u}` and extract branch from that
- [ ] Handle multiple push commands chained with `&&` or `;` — currently checks head-5 lines
      but does not iterate and check each one independently
- [ ] Add `--dry-run` flag to `check` — print decision without side effects, for debugging
- [ ] Write tests — unit tests for state.rs (track, authorize, revoke, is_tracked,
      is_authorized, deduplication); integration tests for the check exit codes

---

## Phase 2 — Hook improvements

- [ ] Ship the hook script as part of the crate — `push-guard install-hook` command that
      writes the hook to `~/.claude/hooks/block-main-push.sh` and sets +x
- [ ] `push-guard uninstall-hook` — remove the hook
- [ ] Detect hook version mismatch — warn if installed hook is older than current push-guard

---

## Phase 3 — UX

- [ ] `push-guard list` — add `--json` output flag for machine consumption
- [ ] `push-guard clean --repo <path>` — remove all tracked/authorized entries for a repo
      (useful when a repo is deleted or renamed)
- [ ] `push-guard clean --stale` — remove entries for repos that no longer exist on disk
- [ ] Color output — green for allowed, red for blocked, yellow for warnings

---

## Non-goals

- No network calls
- No secrets or personal information in state
- No dependency on Claude Code internals — works with any tool that uses pre-tool-use hooks
  that read stdin JSON with `.tool_input.command`

# push-guard — Roadmap

## Phase 0 — Core (complete)

- [x] `hook` — Claude Code PreToolUse entry point
  - Reads stdin JSON, handles all git interception in Rust
  - No shell script intermediary
- [x] `check` — exits 0 (allow) or 1 (block)
- [x] `track` — mark a branch as Claude-created
- [x] `authorize` — grant one-time push authorization
- [x] `revoke` — revoke a previously granted authorization
- [x] `list` — show all tracked and authorized branches
- [x] State persisted at `~/.local/share/push-guard/state.json`
- [x] Claude Code `settings.json` points directly to the binary, no shell script

---

## Phase 1 — Robustness

- [ ] Handle `git push origin HEAD:main` — refspec with colon
  - Currently extracts last token only
  - Refspec destination needs its own parse path
- [ ] Handle `git push` with no remote or branch
  - Look up tracking branch via `git rev-parse --abbrev-ref @{u}`
- [ ] Handle multiple chained push commands
  - `&&` and `;` — check each independently, not just the first
- [ ] `--dry-run` flag on `check` — print decision without side effects
- [ ] Tests
  - Unit: `state.rs` — track, authorize, revoke, is_tracked, is_authorized, deduplication
  - Integration: check exit codes for each authorization scenario

---

## Phase 2 — Registry

- [ ] Define hook package format
  - WASM module exporting `fn handle(tool: &str, command: &str) -> HookResult`
  - `HookResult` variants: allow, block(message), track-branch(name)
- [ ] Embed wasmtime — execute hook behavior WASM modules
- [ ] `push-guard install <package>` — download and register a hook from the registry
- [ ] `push-guard uninstall <package>`
- [ ] `push-guard update` — update all installed hooks
- [ ] Extract current git-push logic into a first-party WASM hook package (`git-push-guard`)
- [ ] Registry server — stores and serves WASM hook packages
- [ ] `push-guard install-claude-hook` — writes `push-guard hook` entry to `~/.claude/settings.json`
- [ ] `push-guard uninstall-claude-hook` — removes the entry
- [ ] Detect version mismatch between binary and installed hook packages

---

## Phase 3 — UX

- [ ] `--json` output flag on `list` — machine-readable output
- [ ] `push-guard clean --repo <path>` — remove all entries for a repo
- [ ] `push-guard clean --stale` — remove entries for repos no longer on disk
- [ ] Color output — green (allowed), red (blocked), yellow (warnings)

---

## Non-goals

- No network calls outside the registry
- No secrets or personal information in state — repo paths, branch names only
- No dependency on Claude Code internals — works with any tool using pre-tool-use hooks with `.tool_input.command`

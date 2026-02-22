# push-guard — Roadmap

---

## Phase 0 — Core (complete)

- [x] `hook` — Claude Code PreToolUse entry point; reads stdin JSON, handles all git
      interception in Rust; no shell script intermediary
- [x] `check` — exits 0 (allow) or 1 (block)
- [x] `track` — mark a branch as Claude-created
- [x] `authorize` — grant one-time push authorization to a branch
- [x] `revoke` — revoke a previously granted authorization
- [x] `list` — show all tracked and authorized branches
- [x] State persisted at `~/.local/share/push-guard/state.json`
- [x] Claude Code settings.json points directly to the binary (`push-guard hook`);
      no shell script

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

## Phase 2 — Registry

Hook behaviors should be installable packages, not hardcoded into the binary.
The binary is the runtime. Behaviors are packages distributed from a registry.

- [ ] Define hook package format — a WASM module that exports:
      `fn handle(tool: &str, command: &str) -> HookResult`
      where `HookResult` is allow / block(message) / track-branch(name)
- [ ] Embed wasmtime — execute hook behavior WASM modules
- [ ] `push-guard install <package>` — download and register a hook behavior from the registry
- [ ] `push-guard uninstall <package>` — remove a hook behavior
- [ ] `push-guard update` — update all installed hook behaviors
- [ ] Extract the current git-push-guard logic into a first-party WASM hook package
      (`@push-guard/git`) published to the registry
- [ ] Registry server — stores and serves WASM hook packages
- [ ] `push-guard install-claude-hook` — writes the `push-guard hook` entry to
      `~/.claude/settings.json` automatically; no manual config needed
- [ ] `push-guard uninstall-claude-hook` — removes the entry from settings.json
- [ ] Detect version mismatch between installed binary and registered hook packages

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

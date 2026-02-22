use assert_cmd::Command;
use tempfile::NamedTempFile;

fn cmd() -> Command {
    Command::cargo_bin("push-guard").unwrap()
}

fn with_state() -> (Command, NamedTempFile) {
    let f = NamedTempFile::new().unwrap();
    let mut c = cmd();
    c.env("PUSH_GUARD_STATE_FILE", f.path());
    (c, f)
}

fn state_cmd(f: &NamedTempFile) -> Command {
    let mut c = cmd();
    c.env("PUSH_GUARD_STATE_FILE", f.path());
    c
}

const REPO: &str = "/tmp/push-guard-test-repo";

// ── Track ─────────────────────────────────────────────────────────────────────

#[test]
fn track_succeeds() {
    let (mut c, _f) = with_state();
    c.args(["track", "--repo", REPO, "--branch", "feature"])
        .assert()
        .success();
}

// ── Check: tracked branch is allowed ─────────────────────────────────────────

#[test]
fn check_tracked_branch_allowed() {
    let f = NamedTempFile::new().unwrap();

    state_cmd(&f)
        .args(["track", "--repo", REPO, "--branch", "feature"])
        .assert()
        .success();

    state_cmd(&f)
        .args(["check", "--repo", REPO, "--remote", "origin", "--branch", "feature"])
        .assert()
        .success();
}

// ── Check: untracked branch is blocked ───────────────────────────────────────

#[test]
fn check_untracked_branch_blocked() {
    let f = NamedTempFile::new().unwrap();

    // Ensure the branch is not tracked or authorized
    state_cmd(&f)
        .args(["check", "--repo", REPO, "--remote", "origin", "--branch", "untracked-xyz"])
        .assert()
        .failure();
}

// ── Check: authorized branch is allowed ──────────────────────────────────────

#[test]
fn check_authorized_branch_allowed() {
    let f = NamedTempFile::new().unwrap();

    state_cmd(&f)
        .args(["authorize", "--repo", REPO, "--branch", "hotfix"])
        .assert()
        .success();

    state_cmd(&f)
        .args(["check", "--repo", REPO, "--remote", "origin", "--branch", "hotfix"])
        .assert()
        .success();
}

// ── Check: revoked authorization is blocked ───────────────────────────────────

#[test]
fn check_revoked_authorization_blocked() {
    let f = NamedTempFile::new().unwrap();

    state_cmd(&f)
        .args(["authorize", "--repo", REPO, "--branch", "hotfix"])
        .assert()
        .success();

    state_cmd(&f)
        .args(["revoke", "--repo", REPO, "--branch", "hotfix"])
        .assert()
        .success();

    state_cmd(&f)
        .args(["check", "--repo", REPO, "--remote", "origin", "--branch", "hotfix"])
        .assert()
        .failure();
}

// ── Check: force push is blocked ─────────────────────────────────────────────

#[test]
fn check_force_push_blocked() {
    let f = NamedTempFile::new().unwrap();

    // Even a tracked branch is blocked on force push
    state_cmd(&f)
        .args(["track", "--repo", REPO, "--branch", "feature"])
        .assert()
        .success();

    state_cmd(&f)
        .args([
            "check", "--repo", REPO, "--remote", "origin",
            "--branch", "feature", "--force",
        ])
        .assert()
        .failure();
}

// ── Check: dry-run does not exit non-zero ────────────────────────────────────

#[test]
fn check_dry_run_does_not_block() {
    let f = NamedTempFile::new().unwrap();

    state_cmd(&f)
        .args([
            "check", "--dry-run",
            "--repo", REPO, "--remote", "origin", "--branch", "untracked-abc",
        ])
        .assert()
        .success();
}

// ── List: --json flag ─────────────────────────────────────────────────────────

#[test]
fn list_json_output() {
    let f = NamedTempFile::new().unwrap();

    state_cmd(&f)
        .args(["track", "--repo", REPO, "--branch", "feat"])
        .assert()
        .success();

    let output = state_cmd(&f)
        .args(["list", "--repo", REPO, "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("invalid JSON output");
    assert!(json["tracked"].as_array().unwrap().iter().any(|v| v == "feat"));
}

// ── Clean: --repo removes entries ─────────────────────────────────────────────

#[test]
fn clean_repo_removes_entries() {
    let f = NamedTempFile::new().unwrap();

    state_cmd(&f)
        .args(["track", "--repo", REPO, "--branch", "feat"])
        .assert()
        .success();

    state_cmd(&f)
        .args(["clean", "--repo", REPO])
        .assert()
        .success();

    // After clean, check should be blocked again
    state_cmd(&f)
        .args(["check", "--repo", REPO, "--remote", "origin", "--branch", "feat"])
        .assert()
        .failure();
}

// ── Clean: --stale removes nonexistent repos ──────────────────────────────────

#[test]
fn clean_stale_removes_ghost_repos() {
    let f = NamedTempFile::new().unwrap();

    // Use a path that doesn't exist
    let ghost = "/definitely/does/not/exist/repo-for-test";

    state_cmd(&f)
        .args(["track", "--repo", ghost, "--branch", "feat"])
        .assert()
        .success();

    state_cmd(&f)
        .args(["clean", "--stale"])
        .assert()
        .success();

    // After stale clean, the ghost repo's branch should be blocked
    state_cmd(&f)
        .args(["check", "--repo", ghost, "--remote", "origin", "--branch", "feat"])
        .assert()
        .failure();
}

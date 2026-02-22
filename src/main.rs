mod state;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use state::State;
use std::io::{IsTerminal, Read};
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "push-guard",
    about = "Git push authorization manager for Claude Code hooks",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Entry point for Claude Code PreToolUse hook.
    /// Reads JSON from stdin, tracks branch creations, enforces push authorization.
    Hook,

    /// Check if a push to a branch is allowed.
    /// Exits 0 (allow) or 1 (blocked).
    Check {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        remote: String,
        #[arg(long)]
        branch: String,
        #[arg(long, default_value = "false")]
        force: bool,
        /// Print decision without exiting non-zero.
        #[arg(long)]
        dry_run: bool,
    },

    /// Mark a branch as created by Claude.
    Track {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        branch: String,
    },

    /// Grant one-time authorization to push to a branch Claude did not create.
    Authorize {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        branch: String,
    },

    /// Revoke a previously granted authorization.
    Revoke {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        branch: String,
    },

    /// List all tracked and authorized branches.
    List {
        #[arg(long)]
        repo: Option<String>,
        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Remove state entries.
    Clean {
        /// Remove all entries for a specific repo path.
        #[arg(long)]
        repo: Option<String>,
        /// Remove entries for repos no longer present on disk.
        #[arg(long)]
        stale: bool,
    },
}

struct PushInfo {
    remote: String,
    branch: String,
    force: bool,
}

// ── Color helpers ─────────────────────────────────────────────────────────────

fn ansi(s: &str, code: &str) -> String {
    if std::io::stderr().is_terminal() {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

fn ansi_stdout(s: &str, code: &str) -> String {
    if std::io::stdout().is_terminal() {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

fn red(s: &str) -> String {
    ansi(s, "31")
}

// ── Git command parsing ───────────────────────────────────────────────────────

/// Returns all branch names created in the command (handles chained commands).
fn detect_branch_creations(command: &str) -> Vec<String> {
    let mut branches = Vec::new();
    for segment in command.split(|c| c == ';' || c == '&') {
        let tokens: Vec<&str> = segment.split_whitespace().collect();
        let mut i = 0;
        while i + 1 < tokens.len() {
            if tokens[i] != "git" {
                i += 1;
                continue;
            }
            match tokens[i + 1] {
                "checkout" | "switch" => {
                    let rest = &tokens[i + 2..];
                    let creates = rest.iter().any(|t| {
                        matches!(*t, "-b" | "-B" | "-c" | "-C")
                            || t.starts_with("-b")
                            || t.starts_with("-B")
                            || t.starts_with("-c")
                            || t.starts_with("-C")
                    });
                    if creates {
                        if let Some(b) = rest.iter().filter(|t| !t.starts_with('-')).last() {
                            branches.push(b.to_string());
                        }
                    }
                }
                "branch" => {
                    if let Some(b) =
                        tokens[i + 2..].iter().find(|t| !t.starts_with('-'))
                    {
                        branches.push(b.to_string());
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
    branches
}

/// Returns all push operations found in the command (handles chained commands).
fn detect_all_pushes(command: &str) -> Vec<PushInfo> {
    let mut pushes = Vec::new();
    for segment in command.split(|c| c == ';' || c == '&') {
        let tokens: Vec<&str> = segment.split_whitespace().collect();
        let mut i = 0;
        while i + 1 < tokens.len() {
            if tokens[i] == "git" && tokens[i + 1] == "push" {
                pushes.push(parse_push_args(&tokens[i + 2..]));
                break;
            }
            i += 1;
        }
    }
    pushes
}

fn parse_push_args(args: &[&str]) -> PushInfo {
    let mut force = false;
    let mut positional: Vec<&str> = vec![];

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        match arg {
            "--force" | "-f" | "--force-with-lease" | "--force-if-includes" => {
                force = true;
            }
            "-o" | "--push-option" | "--receive-pack" | "--exec" => {
                i += 1; // these flags consume the next token
            }
            a if a.starts_with('-') => {}
            _ => positional.push(arg),
        }
        i += 1;
    }

    let (remote, branch) = if positional.is_empty() {
        // No explicit remote or branch — look up the configured upstream
        get_tracking_info()
            .unwrap_or_else(|| ("origin".to_string(), get_current_branch().unwrap_or_default()))
    } else {
        let remote = positional[0].to_string();
        let branch = positional
            .get(1)
            .map(|s| {
                // Handle refspecs: HEAD:main, feature:upstream — take the destination side
                if let Some(colon) = s.find(':') {
                    s[colon + 1..].to_string()
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| get_current_branch().unwrap_or_default());
        (remote, branch)
    };

    PushInfo { remote, branch, force }
}

// ── Git helpers ───────────────────────────────────────────────────────────────

fn get_repo_root() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn get_current_branch() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Returns (remote, branch) from the current tracking upstream.
/// `git rev-parse --abbrev-ref @{u}` → "origin/main" → ("origin", "main")
fn get_tracking_info() -> Option<(String, String)> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "@{u}"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let (remote, branch) = s.split_once('/')?;
    Some((remote.to_string(), branch.to_string()))
}

/// Resolves the actual default branch of a remote — what the remote's HEAD points to.
/// Does not rely on branch name conventions.
///
/// Strategy:
///   1. `git symbolic-ref refs/remotes/<remote>/HEAD` — local, instant, works after fetch
///   2. `git remote show <remote>` — makes a network call, always accurate
///   3. None — caller treats as non-default
fn get_default_branch(remote: &str) -> Option<String> {
    let sym_ref = format!("refs/remotes/{}/HEAD", remote);
    let output = Command::new("git")
        .args(["symbolic-ref", &sym_ref, "--short"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;

    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !s.is_empty() {
        return s
            .strip_prefix(&format!("{}/", remote))
            .map(|b| b.to_string());
    }

    let output = Command::new("git")
        .args(["remote", "show", remote])
        .output()
        .ok()
        .filter(|o| o.status.success())?;

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("HEAD branch:")
                .map(|b| b.trim().to_string())
        })
}

// ── Authorization logic ───────────────────────────────────────────────────────

enum Decision {
    Allow,
    Block(String),
}

fn evaluate(repo: &str, remote: &str, branch: &str, force: bool) -> Result<Decision> {
    if branch.is_empty() {
        return Ok(Decision::Allow);
    }

    if force {
        return Ok(Decision::Block(format!(
            "Force push to '{}' requires explicit user authorization.\n\
             Say \"I authorize\" to proceed.",
            branch
        )));
    }

    let default_branch = get_default_branch(remote);
    if default_branch.as_deref() == Some(branch) {
        return Ok(Decision::Block(format!(
            "'{}' is the default branch of '{}'.\n\
             Recommendation: push to a feature branch instead.\n\
             To push to '{}' directly, say \"I authorize\".",
            branch, remote, branch
        )));
    }

    let state = State::load()?;
    if state.is_tracked(repo, branch) || state.is_authorized(repo, branch) {
        return Ok(Decision::Allow);
    }

    Ok(Decision::Block(format!(
        "Branch '{}' was not created by me and has no authorization.\n\
         To authorize: say \"authorize push to {}\"\n\
         To revoke later: push-guard revoke --repo '{}' --branch '{}'",
        branch, branch, repo, branch
    )))
}

fn check(repo: &str, remote: &str, branch: &str, force: bool, dry_run: bool) -> Result<()> {
    match evaluate(repo, remote, branch, force)? {
        Decision::Allow => {
            if dry_run {
                eprintln!("ALLOWED: push to '{}'", branch);
            }
        }
        Decision::Block(msg) => {
            eprintln!("{}: {}", red("BLOCKED"), msg);
            if !dry_run {
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

// ── Hook entry point ──────────────────────────────────────────────────────────

fn run_hook() -> Result<()> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("Failed to read hook stdin")?;

    let json: serde_json::Value =
        serde_json::from_str(&input).context("Failed to parse hook JSON")?;

    let command = json["tool_input"]["command"]
        .as_str()
        .unwrap_or("")
        .to_string();

    if command.is_empty() {
        return Ok(());
    }

    let repo = get_repo_root().unwrap_or_else(|| "unknown".to_string());

    // Track all branch creations first
    let creations = detect_branch_creations(&command);
    if !creations.is_empty() {
        if let Ok(mut state) = State::load() {
            for branch in &creations {
                state.track(&repo, branch);
            }
            let _ = state.save();
        }
    }

    // Check every push in the command — if any would block, block
    for push in detect_all_pushes(&command) {
        check(&repo, &push.remote, &push.branch, push.force, false)?;
    }

    Ok(())
}

// ── CLI dispatch ──────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hook => {
            if let Err(e) = run_hook() {
                eprintln!("push-guard hook error: {}", e);
            }
        }

        Commands::Check { repo, remote, branch, force, dry_run } => {
            check(&repo, &remote, &branch, force, dry_run)?;
        }

        Commands::Track { repo, branch } => {
            let mut state = State::load()?;
            state.track(&repo, &branch);
            state.save()?;
            eprintln!("Tracking '{}' in '{}'", branch, repo);
        }

        Commands::Authorize { repo, branch } => {
            let mut state = State::load()?;
            state.authorize(&repo, &branch);
            state.save()?;
            eprintln!("Authorized push to '{}' in '{}'", branch, repo);
        }

        Commands::Revoke { repo, branch } => {
            let mut state = State::load()?;
            state.revoke(&repo, &branch);
            state.save()?;
            eprintln!("Revoked authorization for '{}' in '{}'", branch, repo);
        }

        Commands::List { repo, json } => {
            let state = State::load()?;
            if json {
                let output = match &repo {
                    Some(r) => serde_json::json!({
                        "tracked": state.tracked.get(r).cloned().unwrap_or_default(),
                        "authorized": state.authorized.get(r).cloned().unwrap_or_default(),
                    }),
                    None => serde_json::json!({
                        "tracked": state.tracked,
                        "authorized": state.authorized,
                    }),
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                let tag_claude = ansi_stdout("[claude]    ", "32");
                let tag_auth = ansi_stdout("[authorized]", "33");
                match &repo {
                    Some(r) => {
                        for b in state.tracked.get(r).into_iter().flatten() {
                            println!("{}  {}", tag_claude, b);
                        }
                        for b in state.authorized.get(r).into_iter().flatten() {
                            println!("{}  {}", tag_auth, b);
                        }
                    }
                    None => {
                        for (r, branches) in &state.tracked {
                            for b in branches {
                                println!("{}  {}  ::  {}", tag_claude, r, b);
                            }
                        }
                        for (r, branches) in &state.authorized {
                            for b in branches {
                                println!("{}  {}  ::  {}", tag_auth, r, b);
                            }
                        }
                    }
                }
            }
        }

        Commands::Clean { repo, stale } => {
            let mut state = State::load()?;
            let mut changed = false;
            if let Some(r) = repo {
                state.clean_repo(&r);
                eprintln!("Removed all entries for '{}'", r);
                changed = true;
            }
            if stale {
                let removed = state.clean_stale();
                if removed.is_empty() {
                    eprintln!("No stale entries found.");
                } else {
                    for r in &removed {
                        eprintln!("Removed stale repo: {}", r);
                    }
                    changed = true;
                }
            }
            if changed {
                state.save()?;
            }
        }
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // parse_push_args

    #[test]
    fn parse_push_simple() {
        let args = ["origin", "main"];
        let p = parse_push_args(&args);
        assert_eq!(p.remote, "origin");
        assert_eq!(p.branch, "main");
        assert!(!p.force);
    }

    #[test]
    fn parse_push_refspec_colon() {
        let args = ["origin", "HEAD:main"];
        let p = parse_push_args(&args);
        assert_eq!(p.remote, "origin");
        assert_eq!(p.branch, "main");
    }

    #[test]
    fn parse_push_force_flag() {
        let args = ["--force", "origin", "feature"];
        let p = parse_push_args(&args);
        assert_eq!(p.remote, "origin");
        assert_eq!(p.branch, "feature");
        assert!(p.force);
    }

    #[test]
    fn parse_push_force_with_lease() {
        let args = ["origin", "feature", "--force-with-lease"];
        let p = parse_push_args(&args);
        assert!(p.force);
    }

    #[test]
    fn parse_push_short_force() {
        let args = ["-f", "origin", "feature"];
        let p = parse_push_args(&args);
        assert!(p.force);
    }

    // detect_branch_creations

    #[test]
    fn detect_checkout_b() {
        let branches = detect_branch_creations("git checkout -b feature");
        assert_eq!(branches, vec!["feature"]);
    }

    #[test]
    fn detect_switch_c() {
        let branches = detect_branch_creations("git switch -c new-feature");
        assert_eq!(branches, vec!["new-feature"]);
    }

    #[test]
    fn detect_branch_create() {
        let branches = detect_branch_creations("git branch my-branch");
        assert_eq!(branches, vec!["my-branch"]);
    }

    #[test]
    fn detect_chained_multiple_creations() {
        let branches = detect_branch_creations("git branch a; git checkout -b b");
        assert_eq!(branches, vec!["a", "b"]);
    }

    #[test]
    fn detect_no_creation() {
        let branches = detect_branch_creations("git push origin main");
        assert!(branches.is_empty());
    }

    // detect_all_pushes

    #[test]
    fn detect_single_push() {
        let pushes = detect_all_pushes("git push origin feature");
        assert_eq!(pushes.len(), 1);
        assert_eq!(pushes[0].remote, "origin");
        assert_eq!(pushes[0].branch, "feature");
    }

    #[test]
    fn detect_chained_pushes() {
        let pushes = detect_all_pushes("git push origin a; git push upstream b");
        assert_eq!(pushes.len(), 2);
        assert_eq!(pushes[0].remote, "origin");
        assert_eq!(pushes[0].branch, "a");
        assert_eq!(pushes[1].remote, "upstream");
        assert_eq!(pushes[1].branch, "b");
    }

    #[test]
    fn detect_push_with_creation() {
        // Both a branch creation and a push in same chained command
        let creations = detect_branch_creations("git checkout -b feat && git push origin feat");
        assert_eq!(creations, vec!["feat"]);
        let pushes = detect_all_pushes("git checkout -b feat && git push origin feat");
        assert_eq!(pushes.len(), 1);
        assert_eq!(pushes[0].branch, "feat");
    }
}

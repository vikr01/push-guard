mod state;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use state::State;
use std::io::Read;
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
    /// Reads JSON from stdin, tracks branch creation, enforces push authorization.
    Hook,

    /// Check if a push to a branch is allowed.
    /// Exits 0 (allow) or 1 (blocked — hook exits 2 for Claude Code soft block).
    Check {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        branch: String,
        #[arg(long, default_value = "false")]
        force: bool,
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
    },
}

const PROTECTED: &[&str] = &["main", "master", "trunk", "develop"];

struct PushInfo {
    branch: String,
    force: bool,
}

// ── Git command parsing ──────────────────────────────────────────────────────

fn detect_branch_creation(command: &str) -> Option<String> {
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
                        // Branch name is the last non-flag token
                        return rest.iter().filter(|t| !t.starts_with('-')).last().map(|s| s.to_string());
                    }
                }
                "branch" => {
                    // git branch <name> — track it even though it doesn't switch
                    return tokens[i + 2..]
                        .iter()
                        .find(|t| !t.starts_with('-'))
                        .map(|s| s.to_string());
                }
                _ => {}
            }
            i += 1;
        }
    }
    None
}

fn detect_push(command: &str) -> Option<PushInfo> {
    for segment in command.split(|c| c == ';' || c == '&') {
        let tokens: Vec<&str> = segment.split_whitespace().collect();
        let mut i = 0;
        while i + 1 < tokens.len() {
            if tokens[i] == "git" && tokens[i + 1] == "push" {
                return Some(parse_push_args(&tokens[i + 2..]));
            }
            i += 1;
        }
    }
    None
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
            // Flags that take a value argument
            "-o" | "--push-option" | "--receive-pack" | "--exec" => {
                i += 1; // skip value
            }
            a if a.starts_with('-') => {
                // Other flags (--tags, --all, -u, --set-upstream, etc.) — no value
            }
            _ => {
                positional.push(arg);
            }
        }
        i += 1;
    }

    // positional[0] = remote, positional[1] = branch (or refspec)
    let branch = positional.get(1).map(|s| {
        // Handle refspecs like HEAD:main or feature:main
        if let Some(colon) = s.find(':') {
            s[colon + 1..].to_string()
        } else {
            s.to_string()
        }
    });

    // If no explicit branch, look up current tracking branch
    let branch = branch.unwrap_or_else(|| get_current_branch().unwrap_or_default());

    PushInfo { branch, force }
}

// ── Git helpers ──────────────────────────────────────────────────────────────

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

// ── Authorization logic ──────────────────────────────────────────────────────

fn check(repo: &str, branch: &str, force: bool) -> Result<()> {
    if branch.is_empty() {
        std::process::exit(0);
    }

    if force {
        eprintln!(
            "BLOCKED: Force push to '{}' requires explicit user authorization.\n\
             Say \"I authorize\" to proceed.",
            branch
        );
        std::process::exit(1);
    }

    if PROTECTED.contains(&branch) {
        eprintln!(
            "BLOCKED: '{}' is a protected branch.\n\
             Recommendation: push to a feature branch instead.\n\
             To push to '{}' directly, say \"I authorize\".",
            branch, branch
        );
        std::process::exit(1);
    }

    let state = State::load()?;

    if state.is_tracked(repo, branch) || state.is_authorized(repo, branch) {
        std::process::exit(0);
    }

    eprintln!(
        "BLOCKED: Branch '{}' was not created by me and has no authorization.\n\
         To authorize: say \"authorize push to {}\"\n\
         To revoke later: push-guard revoke --repo '{}' --branch '{}'",
        branch, branch, repo, branch
    );
    std::process::exit(1);
}

// ── Hook entry point ─────────────────────────────────────────────────────────

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

    // Branch creation — track and allow
    if let Some(branch) = detect_branch_creation(&command) {
        if let Ok(mut state) = State::load() {
            state.track(&repo, &branch);
            let _ = state.save();
        }
        return Ok(());
    }

    // Push — check authorization
    if let Some(push) = detect_push(&command) {
        check(&repo, &push.branch, push.force)?;
    }

    Ok(())
}

// ── CLI dispatch ─────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hook => {
            if let Err(e) = run_hook() {
                eprintln!("push-guard hook error: {}", e);
                // Don't block on internal errors — let the command through
            }
        }

        Commands::Check { repo, branch, force } => {
            check(&repo, &branch, force)?;
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

        Commands::List { repo } => {
            let state = State::load()?;
            match repo {
                Some(r) => {
                    for b in state.tracked.get(&r).into_iter().flatten() {
                        println!("[claude]     {}", b);
                    }
                    for b in state.authorized.get(&r).into_iter().flatten() {
                        println!("[authorized] {}", b);
                    }
                }
                None => {
                    for (repo, branches) in &state.tracked {
                        for b in branches {
                            println!("[claude]     {}  ::  {}", repo, b);
                        }
                    }
                    for (repo, branches) in &state.authorized {
                        for b in branches {
                            println!("[authorized] {}  ::  {}", repo, b);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

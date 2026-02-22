mod state;

use anyhow::Result;
use clap::{Parser, Subcommand};
use state::State;

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
    /// Check if a push to a branch is allowed.
    /// Exits 0 (allow) or 1 (blocked — hook should exit 2 for Claude Code soft block).
    Check {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        branch: String,
        #[arg(long, default_value = "false")]
        force: bool,
    },
    /// Mark a branch as created by Claude. Called automatically by the hook on git checkout -b.
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

const MAIN_BRANCHES: &[&str] = &["main", "master", "trunk", "develop"];

fn is_protected(branch: &str) -> bool {
    MAIN_BRANCHES.contains(&branch)
}

fn check(repo: &str, branch: &str, force: bool) -> Result<()> {
    if force {
        eprintln!(
            "BLOCKED: Force push to '{}' requires explicit user authorization.\n\
             Say \"I authorize\" to proceed.",
            branch
        );
        std::process::exit(1);
    }

    if is_protected(branch) {
        eprintln!(
            "BLOCKED: '{}' is a protected branch.\n\
             Recommendation: push to a feature branch instead.\n\
             To push to '{}' directly, say \"I authorize\".",
            branch, branch
        );
        std::process::exit(1);
    }

    let state = State::load()?;

    if state.is_tracked(repo, branch) {
        // Claude created this branch — allow freely
        std::process::exit(0);
    }

    if state.is_authorized(repo, branch) {
        // User granted one-time authorization
        std::process::exit(0);
    }

    eprintln!(
        "BLOCKED: Branch '{}' was not created by me and has no authorization.\n\
         To authorize a one-time push: say \"authorize push to {}\"\n\
         To revoke later: push-guard revoke --repo '{}' --branch '{}'",
        branch, branch, repo, branch
    );
    std::process::exit(1);
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
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
                    let tracked = state.tracked.get(&r);
                    let authorized = state.authorized.get(&r);
                    println!("Repo: {}", r);
                    if let Some(branches) = tracked {
                        for b in branches {
                            println!("  [claude]     {}", b);
                        }
                    }
                    if let Some(branches) = authorized {
                        for b in branches {
                            println!("  [authorized] {}", b);
                        }
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

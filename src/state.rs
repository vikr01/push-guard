use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct State {
    /// Branches created by Claude, keyed by canonical repo path
    pub tracked: HashMap<String, Vec<String>>,
    /// One-time authorized branches, keyed by canonical repo path
    pub authorized: HashMap<String, Vec<String>>,
}

pub fn state_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()))
        .join("push-guard")
        .join("state.json")
}

impl State {
    pub fn load() -> Result<Self> {
        let path = state_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read state from {}", path.display()))?;
        serde_json::from_str(&contents).context("Failed to parse state file")
    }

    pub fn save(&self) -> Result<()> {
        let path = state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create dir {}", parent.display()))?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)
            .with_context(|| format!("Failed to write state to {}", path.display()))
    }

    pub fn is_tracked(&self, repo: &str, branch: &str) -> bool {
        self.tracked
            .get(repo)
            .map(|branches| branches.iter().any(|b| b == branch))
            .unwrap_or(false)
    }

    pub fn is_authorized(&self, repo: &str, branch: &str) -> bool {
        self.authorized
            .get(repo)
            .map(|branches| branches.iter().any(|b| b == branch))
            .unwrap_or(false)
    }

    pub fn track(&mut self, repo: &str, branch: &str) {
        let branches = self.tracked.entry(repo.to_string()).or_default();
        if !branches.iter().any(|b| b == branch) {
            branches.push(branch.to_string());
        }
    }

    pub fn authorize(&mut self, repo: &str, branch: &str) {
        let branches = self.authorized.entry(repo.to_string()).or_default();
        if !branches.iter().any(|b| b == branch) {
            branches.push(branch.to_string());
        }
    }

    pub fn revoke(&mut self, repo: &str, branch: &str) {
        if let Some(branches) = self.authorized.get_mut(repo) {
            branches.retain(|b| b != branch);
        }
    }
}

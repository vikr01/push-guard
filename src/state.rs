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
    // Allow overriding the state file path (used in tests)
    if let Ok(p) = std::env::var("PUSH_GUARD_STATE_FILE") {
        return PathBuf::from(p);
    }
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
        if contents.trim().is_empty() {
            return Ok(Self::default());
        }
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

    pub fn clean_repo(&mut self, repo: &str) {
        self.tracked.remove(repo);
        self.authorized.remove(repo);
    }

    /// Removes entries for repo paths that no longer exist on disk.
    /// Returns the list of removed repo paths.
    pub fn clean_stale(&mut self) -> Vec<String> {
        let mut removed: Vec<String> = Vec::new();
        let stale: Vec<String> = self
            .tracked
            .keys()
            .chain(self.authorized.keys())
            .filter(|r| !std::path::Path::new(r.as_str()).exists())
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        for repo in stale {
            self.tracked.remove(&repo);
            self.authorized.remove(&repo);
            removed.push(repo);
        }
        removed
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> State {
        State::default()
    }

    #[test]
    fn fresh_state_not_tracked() {
        let s = empty();
        assert!(!s.is_tracked("/repo", "main"));
    }

    #[test]
    fn fresh_state_not_authorized() {
        let s = empty();
        assert!(!s.is_authorized("/repo", "main"));
    }

    #[test]
    fn track_then_is_tracked() {
        let mut s = empty();
        s.track("/repo", "feature");
        assert!(s.is_tracked("/repo", "feature"));
    }

    #[test]
    fn track_does_not_affect_authorized() {
        let mut s = empty();
        s.track("/repo", "feature");
        assert!(!s.is_authorized("/repo", "feature"));
    }

    #[test]
    fn authorize_then_is_authorized() {
        let mut s = empty();
        s.authorize("/repo", "main");
        assert!(s.is_authorized("/repo", "main"));
    }

    #[test]
    fn revoke_removes_authorization() {
        let mut s = empty();
        s.authorize("/repo", "main");
        s.revoke("/repo", "main");
        assert!(!s.is_authorized("/repo", "main"));
    }

    #[test]
    fn revoke_does_not_affect_tracking() {
        let mut s = empty();
        s.track("/repo", "feature");
        s.revoke("/repo", "feature"); // revoke only affects authorized, not tracked
        assert!(s.is_tracked("/repo", "feature"));
    }

    #[test]
    fn track_deduplication() {
        let mut s = empty();
        s.track("/repo", "feature");
        s.track("/repo", "feature");
        assert_eq!(s.tracked["/repo"].len(), 1);
    }

    #[test]
    fn authorize_deduplication() {
        let mut s = empty();
        s.authorize("/repo", "main");
        s.authorize("/repo", "main");
        assert_eq!(s.authorized["/repo"].len(), 1);
    }

    #[test]
    fn track_multiple_branches() {
        let mut s = empty();
        s.track("/repo", "a");
        s.track("/repo", "b");
        assert!(s.is_tracked("/repo", "a"));
        assert!(s.is_tracked("/repo", "b"));
    }

    #[test]
    fn track_multiple_repos() {
        let mut s = empty();
        s.track("/repo-a", "feature");
        s.track("/repo-b", "feature");
        assert!(s.is_tracked("/repo-a", "feature"));
        assert!(s.is_tracked("/repo-b", "feature"));
        assert!(!s.is_tracked("/repo-a", "other"));
    }

    #[test]
    fn clean_repo_removes_tracked_and_authorized() {
        let mut s = empty();
        s.track("/repo", "a");
        s.authorize("/repo", "b");
        s.clean_repo("/repo");
        assert!(!s.is_tracked("/repo", "a"));
        assert!(!s.is_authorized("/repo", "b"));
    }

    #[test]
    fn clean_repo_does_not_affect_other_repos() {
        let mut s = empty();
        s.track("/repo-a", "feature");
        s.track("/repo-b", "feature");
        s.clean_repo("/repo-a");
        assert!(!s.is_tracked("/repo-a", "feature"));
        assert!(s.is_tracked("/repo-b", "feature"));
    }

    #[test]
    fn clean_stale_removes_nonexistent_repos() {
        let mut s = empty();
        s.track("/definitely/does/not/exist/on/disk/repo", "feature");
        let removed = s.clean_stale();
        assert_eq!(removed.len(), 1);
        assert!(s.tracked.is_empty());
    }

    #[test]
    fn clean_stale_keeps_existing_repos() {
        let mut s = empty();
        s.track("/tmp", "feature"); // /tmp always exists
        let removed = s.clean_stale();
        assert!(removed.is_empty());
        assert!(s.is_tracked("/tmp", "feature"));
    }
}

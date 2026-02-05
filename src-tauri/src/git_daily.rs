use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::process::Command;

use crate::models::{GitDailyResult, GitIdentity};

pub fn collect_git_daily(paths: &[String], identities: &[GitIdentity]) -> Vec<GitDailyResult> {
    let matcher = IdentityMatcher::new(identities);
    paths
        .iter()
        .map(|path| collect_single(path, &matcher))
        .collect()
}

fn collect_single(path: &str, matcher: &IdentityMatcher) -> GitDailyResult {
    let repo_root = Path::new(path);
    if !repo_root.join(".git").exists() {
        return GitDailyResult {
            path: path.to_string(),
            git_daily: None,
            error: None,
        };
    }

    let output = Command::new("git")
        .args(["log", "--pretty=format:%an%x1f%ae%x1f%cd", "--date=short"])
        .current_dir(repo_root)
        .output();

    let output = match output {
        Ok(output) => output,
        Err(err) => {
            return GitDailyResult {
                path: path.to_string(),
                git_daily: None,
                error: Some(format!("执行 git log 失败: {err}")),
            }
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return GitDailyResult {
            path: path.to_string(),
            git_daily: None,
            error: Some(format!("git log 返回失败: {stderr}")),
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();

    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split('\u{1f}');
        let name = parts.next().unwrap_or("");
        let email = parts.next().unwrap_or("");
        let date = parts.next().unwrap_or("").trim();
        if date.is_empty() {
            continue;
        }
        if !matcher.matches(name, email) {
            continue;
        }
        *counts.entry(date.to_string()).or_insert(0) += 1;
    }

    if counts.is_empty() {
        return GitDailyResult {
            path: path.to_string(),
            git_daily: None,
            error: None,
        };
    }

    let git_daily = counts
        .iter()
        .map(|(date, count)| format!("{date}:{count}"))
        .collect::<Vec<_>>()
        .join(",");

    GitDailyResult {
        path: path.to_string(),
        git_daily: Some(git_daily),
        error: None,
    }
}

struct IdentityMatcher {
    tokens: HashSet<String>,
}

impl IdentityMatcher {
    fn new(identities: &[GitIdentity]) -> Self {
        let mut tokens = HashSet::new();
        for identity in identities {
            if let Some(value) = normalize_identity_value(&identity.name) {
                tokens.insert(value);
            }
            if let Some(value) = normalize_identity_value(&identity.email) {
                tokens.insert(value);
            }
        }
        Self { tokens }
    }

    fn matches(&self, name: &str, email: &str) -> bool {
        if self.tokens.is_empty() {
            return true;
        }
        let normalized_name = normalize_identity_value(name);
        let normalized_email = normalize_identity_value(email);
        normalized_name
            .as_ref()
            .map(|value| self.tokens.contains(value))
            .unwrap_or(false)
            || normalized_email
                .as_ref()
                .map(|value| self.tokens.contains(value))
                .unwrap_or(false)
    }
}

fn normalize_identity_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_lowercase())
    }
}

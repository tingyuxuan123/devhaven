use std::path::Path;
use std::process::Command;

use crate::models::BranchListItem;

/// 列出仓库下所有分支名称。
pub fn list_branches(base_path: &str) -> Vec<BranchListItem> {
    if !is_git_repo(base_path) {
        return Vec::new();
    }

    let result = execute_git_command(base_path, &["branch", "--list"]);
    if !result.success {
        return Vec::new();
    }

    let branches: Vec<String> = result
        .output
        .lines()
        .map(|line| line.replace('*', "").trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    let default_branch = resolve_default_branch(&branches, base_path);

    branches
        .into_iter()
        .map(|name| BranchListItem {
            is_main: default_branch.as_deref() == Some(name.as_str()),
            name,
        })
        .collect()
}

// 执行 Git 命令并统一输出格式。
fn execute_git_command(path: &str, args: &[&str]) -> GitCommandResult {
    let output = Command::new("/usr/bin/git")
        .args(args)
        .current_dir(path)
        .output()
        .or_else(|_| Command::new("git").args(args).current_dir(path).output());

    match output {
        Ok(output) => {
            let success = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let combined = if success {
                stdout
            } else if stderr.is_empty() {
                stdout
            } else {
                format!("{stdout}\n{stderr}").trim().to_string()
            };
            GitCommandResult {
                success,
                output: combined,
            }
        }
        Err(err) => GitCommandResult {
            success: false,
            output: format!("执行命令失败: {err}"),
        },
    }
}

// 判断路径是否为 Git 仓库。
fn is_git_repo(path: &str) -> bool {
    Path::new(path).join(".git").exists()
}

fn resolve_default_branch(branches: &[String], base_path: &str) -> Option<String> {
    let symbolic = execute_git_command(base_path, &["symbolic-ref", "refs/remotes/origin/HEAD"]);
    if symbolic.success {
        if let Some(name) = symbolic.output.split('/').last() {
            let name = name.trim();
            if branches.iter().any(|branch| branch == name) {
                return Some(name.to_string());
            }
        }
    }

    if branches.iter().any(|branch| branch == "main") {
        return Some("main".to_string());
    }
    if branches.iter().any(|branch| branch == "master") {
        return Some("master".to_string());
    }

    None
}

struct GitCommandResult {
    success: bool,
    output: String,
}

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::models::Project;
use crate::time_utils::{now_swift, system_time_to_swift, system_time_to_unix_seconds};

/// 根据目录列表扫描可用项目路径。
pub fn discover_projects(directories: &[String]) -> Vec<String> {
    let mut all_paths = Vec::new();
    for directory in directories {
        let mut found = scan_directory_with_git(directory);
        all_paths.append(&mut found);
    }
    all_paths.sort();
    all_paths.dedup();
    all_paths
}

/// 构建项目列表，复用已有数据并更新元信息。
pub fn build_projects(paths: &[String], existing: &[Project]) -> Vec<Project> {
    let mut existing_by_path: HashMap<&str, &Project> = HashMap::new();
    for project in existing {
        existing_by_path.insert(project.path.as_str(), project);
    }

    paths
        .iter()
        .filter_map(|path| create_project(path, &existing_by_path))
        .collect()
}

// 扫描指定目录：收录根目录（若为 Git 仓库）、其直接子目录，以及更深层的 Git 仓库。
fn scan_directory_with_git(path: &str) -> Vec<String> {
    let mut results = Vec::new();
    let root = Path::new(path);
    if !root.exists() {
        return results;
    }

    if is_git_repo(root) {
        if let Some(as_str) = root.to_str() {
            results.push(as_str.to_string());
        }
    }

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return results,
    };

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }

        let entry_path = entry.path();
        let name = entry.file_name();
        if should_skip_direct_dir(&name) {
            continue;
        }

        if let Some(as_str) = entry_path.to_str() {
            results.push(as_str.to_string());
        }

        collect_git_repos(&entry_path, &mut results);
    }

    results
}

fn collect_git_repos(path: &Path, results: &mut Vec<String>) {
    if is_git_repo(path) {
        if let Some(as_str) = path.to_str() {
            results.push(as_str.to_string());
        }
        return;
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        if should_skip_recursive_dir(&name) {
            continue;
        }

        collect_git_repos(&entry.path(), results);
    }
}

fn should_skip_direct_dir(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name.starts_with('.')
}

fn should_skip_recursive_dir(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    if name == ".git" {
        return true;
    }
    if name.starts_with('.') {
        return true;
    }
    matches!(name.as_ref(), "node_modules" | "target" | "dist" | "build")
}

#[cfg(test)]
mod tests {
    use super::scan_directory_with_git;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn scan_directory_with_git_handles_root_and_nested_repos() {
        let root = std::env::temp_dir().join(format!("devhaven_test_{}", uuid::Uuid::new_v4()));
        let sub_a = root.join("alpha");
        let sub_b = root.join("beta");
        let sub_c = root.join("gamma");
        let hidden = root.join(".hidden");
        let nested_repo = sub_a.join("project-x").join(".git");
        let nested_hidden_repo = hidden.join("project-y").join(".git");
        let nested_in_git = sub_b.join("ignored").join(".git");

        fs::create_dir_all(root.join(".git")).expect("create root git");
        fs::create_dir_all(&sub_a).expect("create alpha dir");
        fs::create_dir_all(sub_b.join(".git")).expect("create beta git");
        fs::create_dir_all(&sub_c).expect("create gamma dir");
        fs::create_dir_all(&hidden).expect("create hidden dir");
        fs::create_dir_all(&nested_repo).expect("create nested git");
        fs::create_dir_all(&nested_hidden_repo).expect("create hidden nested git");
        fs::create_dir_all(&nested_in_git).expect("create nested in git");

        let root_str = root.to_string_lossy().to_string();
        let results = scan_directory_with_git(&root_str);
        let result_paths: Vec<PathBuf> = results.into_iter().map(PathBuf::from).collect();

        assert!(result_paths.contains(&root));
        assert!(result_paths.contains(&sub_a));
        assert!(result_paths.contains(&sub_b));
        assert!(result_paths.contains(&sub_c));
        assert!(result_paths.contains(&sub_a.join("project-x")));
        assert!(!result_paths.contains(&hidden));
        assert!(!result_paths.contains(&hidden.join("project-y")));
        assert!(!result_paths.contains(&sub_b.join("ignored")));

        let _ = fs::remove_dir_all(&root);
    }
}

// 创建单个项目模型，必要时复用已存在的配置。
fn create_project(path: &str, existing_by_path: &HashMap<&str, &Project>) -> Option<Project> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_dir() {
        return None;
    }

    let name = Path::new(path)
        .file_name()
        .and_then(|os| os.to_str())
        .unwrap_or(path)
        .to_string();

    let mtime = metadata
        .modified()
        .map(system_time_to_swift)
        .unwrap_or_else(|_| now_swift());

    let unix_mtime = metadata
        .modified()
        .map(system_time_to_unix_seconds)
        .unwrap_or(0.0);
    let size = metadata.len() as i64;
    let checksum = format!("{}_{}", unix_mtime, size);

    let git_info = load_git_info(path);

    if let Some(existing) = existing_by_path.get(path) {
        return Some(Project {
            id: existing.id.clone(),
            name,
            path: path.to_string(),
            tags: existing.tags.clone(),
            mtime,
            size,
            checksum,
            git_commits: git_info.commit_count,
            git_last_commit: git_info.last_commit,
            git_daily: existing.git_daily.clone(),
            created: existing.created,
            checked: now_swift(),
        });
    }

    Some(Project {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        path: path.to_string(),
        tags: Vec::new(),
        mtime,
        size,
        checksum,
        git_commits: git_info.commit_count,
        git_last_commit: git_info.last_commit,
        git_daily: None,
        created: now_swift(),
        checked: now_swift(),
    })
}

struct GitInfo {
    commit_count: i64,
    last_commit: f64,
}

// 读取 Git 信息（提交次数与最后提交时间）。
fn load_git_info(path: &str) -> GitInfo {
    let git_dir = Path::new(path).join(".git");
    if !git_dir.exists() {
        return GitInfo {
            commit_count: 0,
            last_commit: 0.0,
        };
    }

    let last_commit = run_git_command(path, &["log", "--format=%ct", "-n", "1"])
        .and_then(|output| output.trim().parse::<f64>().ok())
        .unwrap_or(0.0);

    let commit_count = run_git_command(path, &["rev-list", "--count", "HEAD"])
        .and_then(|output| output.trim().parse::<i64>().ok())
        .unwrap_or(0);

    GitInfo {
        commit_count,
        last_commit: crate::time_utils::unix_to_swift(last_commit),
    }
}

fn is_git_repo(path: &Path) -> bool {
    path.join(".git").exists()
}

// 执行 Git 命令并返回输出内容。
fn run_git_command(path: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("/usr/bin/git")
        .args(args)
        .current_dir(path)
        .output()
        .or_else(|_| Command::new("git").args(args).current_dir(path).output())
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

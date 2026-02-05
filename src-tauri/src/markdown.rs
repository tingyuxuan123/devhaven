use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crate::models::MarkdownFileEntry;

/// 读取项目内的 Markdown 文件列表。
pub fn list_markdown_files(project_path: &str) -> Result<Vec<MarkdownFileEntry>, String> {
    let root = Path::new(project_path);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    collect_markdown_files(root, root, &mut entries);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

/// 读取项目内指定 Markdown 文件内容。
pub fn read_markdown_file(project_path: &str, relative_path: &str) -> Result<String, String> {
    let root = Path::new(project_path);
    if !root.exists() {
        return Err("项目路径不存在".to_string());
    }
    let relative = Path::new(relative_path);
    if relative.is_absolute() {
        return Err("Markdown 路径无效".to_string());
    }
    let candidate = root.join(relative);
    if !is_markdown_file(&candidate) {
        return Err("仅支持读取 .md 文件".to_string());
    }
    let root_canon = fs::canonicalize(root).map_err(|err| format!("读取项目路径失败: {err}"))?;
    let file_canon =
        fs::canonicalize(&candidate).map_err(|err| format!("读取 Markdown 失败: {err}"))?;
    if !file_canon.starts_with(&root_canon) {
        return Err("Markdown 路径越界".to_string());
    }
    fs::read_to_string(&file_canon).map_err(|err| format!("读取 Markdown 失败: {err}"))
}

fn collect_markdown_files(root: &Path, current: &Path, entries: &mut Vec<MarkdownFileEntry>) {
    let read_dir = match fs::read_dir(current) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };

        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        if file_type.is_dir() {
            let name = entry.file_name();
            if should_skip_dir(&name) {
                continue;
            }
            collect_markdown_files(root, &path, entries);
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        if !is_markdown_file(&path) {
            continue;
        }

        let relative = match path.strip_prefix(root) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let relative_path = normalize_path(relative);
        let absolute_path = path.to_string_lossy().to_string();
        entries.push(MarkdownFileEntry {
            path: relative_path,
            absolute_path,
        });
    }
}

fn should_skip_dir(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    if name.starts_with('.') {
        return true;
    }
    matches!(name.as_ref(), "node_modules" | "target" | "dist" | "build")
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

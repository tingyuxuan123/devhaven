use std::fs;
use std::path::Path;

const NOTES_FILE: &str = "PROJECT_NOTES.md";

/// 读取项目备注内容，空内容返回 None。
pub fn read_notes(project_path: &str) -> Result<Option<String>, String> {
    let notes_path = Path::new(project_path).join(NOTES_FILE);
    if !notes_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&notes_path).map_err(|err| format!("读取备注失败: {err}"))?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(content))
    }
}

/// 写入项目备注内容，传 None 则删除文件。
pub fn write_notes(project_path: &str, notes: Option<String>) -> Result<(), String> {
    let notes_path = Path::new(project_path).join(NOTES_FILE);
    match notes {
        Some(content) => {
            if let Some(parent) = notes_path.parent() {
                fs::create_dir_all(parent).map_err(|err| format!("创建备注目录失败: {err}"))?;
            }
            fs::write(&notes_path, content).map_err(|err| format!("写入备注失败: {err}"))?;
            Ok(())
        }
        None => {
            if notes_path.exists() {
                fs::remove_file(&notes_path).map_err(|err| format!("删除备注失败: {err}"))?;
            }
            Ok(())
        }
    }
}

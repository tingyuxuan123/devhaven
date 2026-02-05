use std::fs;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::models::{AppStateFile, HeatmapCacheFile, Project};

// 获取应用数据目录。
fn app_support_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let home_dir = app
        .path()
        .home_dir()
        .map_err(|err| format!("无法获取用户目录: {err}"))?;
    Ok(home_dir.join(".devhaven"))
}

// 确保目录存在。
fn ensure_dir(path: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|err| format!("无法创建目录: {err}"))
}

// 读取并反序列化 JSON 文件。
fn read_json<T: DeserializeOwned>(path: &PathBuf) -> Result<T, String> {
    let data = fs::read(path).map_err(|err| format!("无法读取文件: {err}"))?;
    serde_json::from_slice(&data).map_err(|err| format!("解析 JSON 失败: {err}"))
}

// 以易读格式写入 JSON。
fn write_json_pretty<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    let data =
        serde_json::to_vec_pretty(value).map_err(|err| format!("序列化 JSON 失败: {err}"))?;
    fs::write(path, data).map_err(|err| format!("写入文件失败: {err}"))
}

// 以紧凑格式写入 JSON。
fn write_json_compact<T: Serialize + ?Sized>(path: &PathBuf, value: &T) -> Result<(), String> {
    let data = serde_json::to_vec(value).map_err(|err| format!("序列化 JSON 失败: {err}"))?;
    fs::write(path, data).map_err(|err| format!("写入文件失败: {err}"))
}

/// 读取应用状态文件。
pub fn load_app_state(app: &AppHandle) -> Result<AppStateFile, String> {
    let dir = app_support_dir(app)?;
    ensure_dir(&dir)?;
    let file_path = dir.join("app_state.json");
    if !file_path.exists() {
        return Ok(AppStateFile::default());
    }
    read_json(&file_path)
}

/// 保存应用状态文件。
pub fn save_app_state(app: &AppHandle, state: &AppStateFile) -> Result<(), String> {
    let dir = app_support_dir(app)?;
    ensure_dir(&dir)?;
    let file_path = dir.join("app_state.json");
    write_json_pretty(&file_path, state)
}

/// 读取项目缓存列表。
pub fn load_projects(app: &AppHandle) -> Result<Vec<Project>, String> {
    let dir = app_support_dir(app)?;
    ensure_dir(&dir)?;
    let file_path = dir.join("projects.json");
    if !file_path.exists() {
        return Ok(Vec::new());
    }
    read_json(&file_path)
}

/// 保存项目缓存列表。
pub fn save_projects(app: &AppHandle, projects: &[Project]) -> Result<(), String> {
    let dir = app_support_dir(app)?;
    ensure_dir(&dir)?;
    let file_path = dir.join("projects.json");
    write_json_compact(&file_path, projects)
}

/// 读取热力图缓存。
pub fn load_heatmap_cache(app: &AppHandle) -> Result<HeatmapCacheFile, String> {
    let dir = app_support_dir(app)?;
    ensure_dir(&dir)?;
    let file_path = dir.join("heatmap_cache.json");
    if !file_path.exists() {
        return Ok(HeatmapCacheFile::default());
    }
    read_json(&file_path)
}

/// 保存热力图缓存。
pub fn save_heatmap_cache(app: &AppHandle, cache: &HeatmapCacheFile) -> Result<(), String> {
    let dir = app_support_dir(app)?;
    ensure_dir(&dir)?;
    let file_path = dir.join("heatmap_cache.json");
    write_json_pretty(&file_path, cache)
}

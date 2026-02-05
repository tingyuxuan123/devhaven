use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Mutex, OnceLock,
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, Utc};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::models::{CodexLastEventType, CodexMessageCounts, CodexSessionSummary};

const CODEX_SESSIONS_DIR: &str = ".codex/sessions";
const MAX_TAIL_LINES: usize = 200;
const MAX_TAIL_BYTES: u64 = 256 * 1024;
const ACTIVE_WINDOW_MS: i64 = 10_000;
const RECENT_FILE_WINDOW_MS: i64 = 5 * 60_000;
const WATCH_DEBOUNCE_MS: u64 = 350;
const CODEX_SESSIONS_EVENT: &str = "codex-sessions-update";

type SessionCache = HashMap<PathBuf, CachedSession>;

static CODEX_SESSION_CACHE: OnceLock<Mutex<SessionCache>> = OnceLock::new();
static CODEX_SESSION_WATCH_STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct CachedSession {
    summary: CodexSessionSummary,
    modified: i64,
    size: u64,
}

pub fn list_sessions(app: &AppHandle) -> Result<Vec<CodexSessionSummary>, String> {
    let base_dir = app
        .path()
        .home_dir()
        .map_err(|err| format!("无法获取用户目录: {err}"))?
        .join(CODEX_SESSIONS_DIR);

    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let files = collect_rollout_files(&base_dir)?;
    let mut seen = HashSet::new();
    let cache = CODEX_SESSION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache
        .lock()
        .map_err(|_| "Codex 会话缓存锁异常".to_string())?;

    let mut sessions = Vec::new();
    let now_ms = Utc::now().timestamp_millis();
    let recent_threshold = now_ms - RECENT_FILE_WINDOW_MS;
    for path in files {
        seen.insert(path.clone());
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(err) => {
                log::warn!(
                    "读取 Codex 会话文件失败: path={} err={}",
                    path.display(),
                    err
                );
                continue;
            }
        };
        let modified = metadata
            .modified()
            .ok()
            .and_then(system_time_to_millis)
            .unwrap_or(0);
        if modified > 0 && modified < recent_threshold {
            continue;
        }
        let size = metadata.len();
        let should_refresh = match cache.get(&path) {
            Some(cached) => cached.modified != modified || cached.size != size,
            None => true,
        };
        if should_refresh {
            match parse_session_file(&path) {
                Ok(summary) => {
                    cache.insert(
                        path.clone(),
                        CachedSession {
                            summary,
                            modified,
                            size,
                        },
                    );
                }
                Err(err) => {
                    log::warn!("解析 Codex 会话失败: path={} err={}", path.display(), err);
                }
            }
        }
    }

    cache.retain(|path, _| seen.contains(path));
    for cached in cache.values() {
        let mut summary = cached.summary.clone();
        summary.is_running = summary.last_activity_at > 0
            && now_ms.saturating_sub(summary.last_activity_at) <= ACTIVE_WINDOW_MS;
        if summary.is_running {
            sessions.push(summary);
        }
    }

    sessions.sort_by(|left, right| right.last_activity_at.cmp(&left.last_activity_at));
    Ok(sessions)
}

pub fn ensure_session_watcher(app: &AppHandle) -> Result<(), String> {
    let base_dir = app
        .path()
        .home_dir()
        .map_err(|err| format!("无法获取用户目录: {err}"))?
        .join(CODEX_SESSIONS_DIR);

    if !base_dir.exists() {
        return Ok(());
    }

    if CODEX_SESSION_WATCH_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }

    let app_handle = app.clone();
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
            Ok(watcher) => watcher,
            Err(error) => {
                log::warn!("启动 Codex 会话监听失败: {}", error);
                return;
            }
        };

        if let Err(error) = watcher.watch(&base_dir, RecursiveMode::Recursive) {
            log::warn!("监听 Codex 会话目录失败: {}", error);
            return;
        }

        event_loop(rx, app_handle);
    });

    Ok(())
}

fn event_loop(rx: Receiver<Result<notify::Event, notify::Error>>, app: AppHandle) {
    let mut pending = false;
    let mut last_emit = std::time::Instant::now()
        .checked_sub(Duration::from_millis(WATCH_DEBOUNCE_MS))
        .unwrap_or_else(std::time::Instant::now);

    loop {
        match rx.recv_timeout(Duration::from_millis(WATCH_DEBOUNCE_MS)) {
            Ok(Ok(event)) => {
                if should_refresh_for_event(&event) {
                    pending = true;
                    if last_emit.elapsed() >= Duration::from_millis(WATCH_DEBOUNCE_MS) {
                        pending = false;
                        last_emit = std::time::Instant::now();
                        emit_sessions(&app);
                    }
                }
            }
            Ok(Err(error)) => {
                log::warn!("Codex 会话监听错误: {}", error);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if pending {
                    pending = false;
                    last_emit = std::time::Instant::now();
                    emit_sessions(&app);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn should_refresh_for_event(event: &notify::Event) -> bool {
    let matches_kind = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    );
    if !matches_kind {
        return false;
    }
    event.paths.iter().any(|path| is_rollout_file(path))
}

fn emit_sessions(app: &AppHandle) {
    match list_sessions(app) {
        Ok(sessions) => {
            if let Err(error) = app.emit(CODEX_SESSIONS_EVENT, sessions) {
                log::warn!("推送 Codex 会话更新失败: {}", error);
            }
        }
        Err(error) => {
            log::warn!("刷新 Codex 会话失败: {}", error);
        }
    }
}

fn collect_rollout_files(base_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let candidate_dirs = collect_candidate_dirs(base_dir);
    if candidate_dirs.is_empty() {
        collect_rollout_files_recursive(base_dir, &mut files)?;
    } else {
        for dir in candidate_dirs {
            collect_rollout_files_shallow(&dir, &mut files)?;
        }
    }
    Ok(files)
}

fn collect_candidate_dirs(base_dir: &Path) -> Vec<PathBuf> {
    let today = Local::now().date_naive();
    let yesterday = today.pred_opt().unwrap_or(today);
    let mut dirs = Vec::new();
    for date in [today, yesterday] {
        let dir = build_date_dir(base_dir, date);
        if dir.exists() && dir.is_dir() {
            dirs.push(dir);
        }
    }
    dirs
}

fn build_date_dir(base_dir: &Path, date: chrono::NaiveDate) -> PathBuf {
    base_dir
        .join(format!("{:04}", date.year()))
        .join(format!("{:02}", date.month()))
        .join(format!("{:02}", date.day()))
}

fn collect_rollout_files_shallow(dir: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("读取目录失败: {err}"))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("读取目录项失败: {err}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if is_rollout_file(&path) {
            output.push(path);
        }
    }
    Ok(())
}

fn collect_rollout_files_recursive(dir: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("读取目录失败: {err}"))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("读取目录项失败: {err}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rollout_files_recursive(&path, output)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if is_rollout_file(&path) {
            output.push(path);
        }
    }
    Ok(())
}

fn is_rollout_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    file_name.starts_with("rollout-") && file_name.ends_with(".jsonl")
}

fn parse_session_file(path: &Path) -> Result<CodexSessionSummary, String> {
    let meta = read_session_meta(path)?;
    let tail_lines = read_tail_lines(path, MAX_TAIL_LINES, MAX_TAIL_BYTES)?;

    let mut last_activity_at = meta.started_at;
    let mut last_user_message: Option<String> = None;
    let mut last_agent_message: Option<String> = None;
    let mut last_user_ts: i64 = 0;
    let mut last_agent_ts: i64 = 0;
    let mut last_event_at: i64 = 0;
    let mut last_event_type: Option<CodexLastEventType> = None;
    let mut counts = CodexMessageCounts { user: 0, agent: 0 };

    for line in tail_lines {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };
        let timestamp = value
            .get("timestamp")
            .and_then(parse_timestamp)
            .unwrap_or(0);
        if timestamp > last_activity_at {
            last_activity_at = timestamp;
        }
        if value.get("type").and_then(|item| item.as_str()) != Some("event_msg") {
            continue;
        }
        let payload = value.get("payload");
        let event_type = payload
            .and_then(|item| item.get("type"))
            .and_then(|item| item.as_str());
        let message = payload
            .and_then(|item| item.get("message"))
            .and_then(|item| item.as_str())
            .map(|text| text.to_string());
        match event_type {
            Some("user_message") => {
                counts.user += 1;
                if let Some(text) = message {
                    if timestamp >= last_user_ts {
                        last_user_ts = timestamp;
                        last_user_message = Some(text);
                    }
                }
                if timestamp >= last_event_at {
                    last_event_at = timestamp;
                    last_event_type = Some(CodexLastEventType::User);
                }
            }
            Some("agent_message") => {
                counts.agent += 1;
                if let Some(text) = message {
                    if timestamp >= last_agent_ts {
                        last_agent_ts = timestamp;
                        last_agent_message = Some(text);
                    }
                }
                if timestamp >= last_event_at {
                    last_event_at = timestamp;
                    last_event_type = Some(CodexLastEventType::Agent);
                }
            }
            _ => {}
        }
    }

    if last_activity_at <= 0 {
        if let Some(modified) = file_modified_millis(path) {
            last_activity_at = modified;
        }
    }

    let now_ms = Utc::now().timestamp_millis();
    let is_running =
        last_activity_at > 0 && now_ms.saturating_sub(last_activity_at) <= ACTIVE_WINDOW_MS;

    Ok(CodexSessionSummary {
        id: meta.id,
        cwd: meta.cwd,
        cli_version: meta.cli_version,
        started_at: meta.started_at,
        last_activity_at,
        is_running,
        last_user_message,
        last_agent_message,
        message_counts: counts,
        last_event_type,
    })
}

struct SessionMeta {
    id: String,
    cwd: String,
    cli_version: Option<String>,
    started_at: i64,
}

fn read_session_meta(path: &Path) -> Result<SessionMeta, String> {
    let file = File::open(path).map_err(|err| format!("读取会话文件失败: {err}"))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|err| format!("读取会话首行失败: {err}"))?;
    if line.trim().is_empty() {
        return Err("会话文件为空".to_string());
    }
    let value: Value =
        serde_json::from_str(&line).map_err(|err| format!("解析会话首行失败: {err}"))?;
    if value.get("type").and_then(|item| item.as_str()) != Some("session_meta") {
        return Err("会话首行不是 session_meta".to_string());
    }
    let payload = value
        .get("payload")
        .ok_or_else(|| "session_meta 缺少 payload".to_string())?;
    let id = payload
        .get("id")
        .and_then(|item| item.as_str())
        .filter(|text| !text.is_empty())
        .map(|text| text.to_string())
        .or_else(|| fallback_session_id(path))
        .ok_or_else(|| "session_meta 缺少 id".to_string())?;
    let cwd = payload
        .get("cwd")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    let cli_version = payload
        .get("cli_version")
        .and_then(|item| item.as_str())
        .map(|text| text.to_string());
    let started_at = payload
        .get("timestamp")
        .and_then(parse_timestamp)
        .or_else(|| value.get("timestamp").and_then(parse_timestamp))
        .or_else(|| file_modified_millis(path))
        .unwrap_or(0);

    Ok(SessionMeta {
        id,
        cwd,
        cli_version,
        started_at,
    })
}

fn read_tail_lines(path: &Path, max_lines: usize, max_bytes: u64) -> Result<Vec<String>, String> {
    let mut file = File::open(path).map_err(|err| format!("读取会话文件失败: {err}"))?;
    let size = file
        .metadata()
        .map_err(|err| format!("读取文件元信息失败: {err}"))?
        .len();
    let start = if size > max_bytes {
        size - max_bytes
    } else {
        0
    };
    file.seek(SeekFrom::Start(start))
        .map_err(|err| format!("定位会话文件失败: {err}"))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|err| format!("读取会话文件失败: {err}"))?;
    let text = String::from_utf8_lossy(&buffer);
    let mut lines: Vec<&str> = text.split('\n').collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let mut trimmed: Vec<String> = lines
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect();
    if trimmed.len() > max_lines {
        trimmed = trimmed.split_off(trimmed.len() - max_lines);
    }
    Ok(trimmed)
}

fn parse_timestamp(value: &Value) -> Option<i64> {
    value
        .as_str()
        .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
        .map(|dt| dt.timestamp_millis())
}

fn fallback_session_id(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(|text| text.to_string())
}

fn file_modified_millis(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    system_time_to_millis(modified)
}

fn system_time_to_millis(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn parse_session_file_extracts_recent_messages() {
        let dir = std::env::temp_dir().join(format!("devhaven-codex-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("rollout-test.jsonl");
        let content = [
            r#"{"timestamp":"2026-01-28T05:07:13.570Z","type":"session_meta","payload":{"id":"abc","timestamp":"2026-01-28T05:07:13.545Z","cwd":"/tmp/project","cli_version":"0.92.0"}}"#,
            r#"{"timestamp":"2026-01-28T05:08:13.000Z","type":"event_msg","payload":{"type":"user_message","message":"hi"}}"#,
            r#"{"timestamp":"2026-01-28T05:08:20.000Z","type":"event_msg","payload":{"type":"agent_message","message":"hello"}}"#,
        ]
        .join("\n");
        fs::write(&path, content).expect("write temp file");

        let summary = parse_session_file(&path).expect("parse session");
        assert_eq!(summary.id, "abc");
        assert_eq!(summary.cwd, "/tmp/project");
        assert_eq!(summary.last_user_message.as_deref(), Some("hi"));
        assert_eq!(summary.last_agent_message.as_deref(), Some("hello"));
        assert_eq!(summary.message_counts.user, 1);
        assert_eq!(summary.message_counts.agent, 1);
        assert!(summary.last_activity_at >= summary.started_at);
    }
}

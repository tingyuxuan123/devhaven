mod codex_sessions;
mod git_daily;
mod git_ops;
mod markdown;
mod models;
mod notes;
mod project_loader;
mod storage;
mod system;
mod terminal;
mod time_utils;

use std::sync::Mutex;
use std::time::Instant;
use tauri::AppHandle;
use tauri::Manager;
use tauri::State;
use tauri_plugin_log::{Target, TargetKind};

use crate::models::{
    AppStateFile, BranchListItem, CodexSessionSummary, DevToolPreset, GitDailyResult, GitIdentity,
    HeatmapCacheFile, MarkdownFileEntry, Project,
};
use crate::system::{EditorOpenParams, TerminalOpenParams};
use crate::terminal::{
    TerminalManager, TerminalSessionInfo, TmuxPaneCursor, TmuxPaneInfo, TmuxSupportStatus,
    TmuxWindowInfo,
};

#[tauri::command]
/// 读取应用状态。
fn load_app_state(app: AppHandle) -> Result<AppStateFile, String> {
    log_command_result("load_app_state", || storage::load_app_state(&app))
}

#[tauri::command]
/// 保存应用状态。
fn save_app_state(app: AppHandle, state: AppStateFile) -> Result<(), String> {
    log_command_result("save_app_state", || storage::save_app_state(&app, &state))
}

#[tauri::command]
/// 读取项目缓存列表。
fn load_projects(app: AppHandle) -> Result<Vec<Project>, String> {
    log_command_result("load_projects", || storage::load_projects(&app))
}

#[tauri::command]
/// 保存项目缓存列表。
fn save_projects(app: AppHandle, projects: Vec<Project>) -> Result<(), String> {
    log_command_result("save_projects", || storage::save_projects(&app, &projects))
}

#[tauri::command]
/// 扫描工作目录，发现项目路径。
fn discover_projects(directories: Vec<String>) -> Vec<String> {
    log_command("discover_projects", || {
        log::info!("discover_projects directories={}", directories.len());
        project_loader::discover_projects(&directories)
    })
}

#[tauri::command]
/// 构建项目列表并补齐元数据。
fn build_projects(paths: Vec<String>, existing: Vec<Project>) -> Vec<Project> {
    log_command("build_projects", || {
        log::info!(
            "build_projects paths={} existing={}",
            paths.len(),
            existing.len()
        );
        project_loader::build_projects(&paths, &existing)
    })
}

#[tauri::command]
/// 获取分支列表。
fn list_branches(base_path: String) -> Vec<BranchListItem> {
    log_command("list_branches", || {
        log::info!("list_branches base_path={}", base_path);
        git_ops::list_branches(&base_path)
    })
}

#[tauri::command]
/// 在文件管理器中定位路径。
fn open_in_finder(path: String) -> Result<(), String> {
    log_command_result("open_in_finder", || {
        log::info!("open_in_finder path={}", path);
        system::open_in_finder(&path)
    })
}

#[tauri::command]
/// 在终端中打开路径。
fn open_in_terminal(params: TerminalOpenParams) -> Result<(), String> {
    log_command_result("open_in_terminal", || {
        log::info!("open_in_terminal path={}", params.path);
        system::open_in_terminal(params)
    })
}

#[tauri::command]
/// 使用外部编辑器打开路径。
fn open_in_editor(params: EditorOpenParams) -> Result<(), String> {
    log_command_result("open_in_editor", || {
        log::info!("open_in_editor path={}", params.path);
        system::open_in_editor(params)
    })
}

#[tauri::command]
/// 列出已检测到的开发工具预设。
fn list_dev_tool_presets() -> Vec<DevToolPreset> {
    log_command("list_dev_tool_presets", system::list_dev_tool_presets)
}

#[tauri::command]
/// 设置指定窗口可在 macOS 全屏空间中作为辅助窗口展示。
fn set_window_fullscreen_auxiliary(
    app: AppHandle,
    window_label: String,
    enabled: bool,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let window = app
            .get_webview_window(&window_label)
            .ok_or_else(|| "窗口不存在".to_string())?;
        return apply_fullscreen_auxiliary(&window, enabled);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, window_label, enabled);
        Ok(())
    }
}

#[tauri::command]
/// 复制文本到剪贴板。
fn copy_to_clipboard(app: AppHandle, content: String) -> Result<(), String> {
    log_command_result("copy_to_clipboard", || {
        log::info!("copy_to_clipboard size={}", content.len());
        system::copy_to_clipboard(&app, &content)
    })
}

#[tauri::command]
/// 读取项目备注内容。
fn read_project_notes(path: String) -> Result<Option<String>, String> {
    log_command_result("read_project_notes", || {
        log::info!("read_project_notes path={}", path);
        notes::read_notes(&path)
    })
}

#[tauri::command]
/// 写入项目备注内容。
fn write_project_notes(path: String, notes: Option<String>) -> Result<(), String> {
    log_command_result("write_project_notes", || {
        let note_len = notes.as_ref().map(|value| value.len()).unwrap_or(0);
        log::info!("write_project_notes path={} size={}", path, note_len);
        notes::write_notes(&path, notes)
    })
}

#[tauri::command]
/// 列出项目内的 Markdown 文件。
fn list_project_markdown_files(path: String) -> Result<Vec<MarkdownFileEntry>, String> {
    log_command_result("list_project_markdown_files", || {
        log::info!("list_project_markdown_files path={}", path);
        markdown::list_markdown_files(&path)
    })
}

#[tauri::command]
/// 读取项目内指定 Markdown 内容。
fn read_project_markdown_file(path: String, relative_path: String) -> Result<String, String> {
    log_command_result("read_project_markdown_file", || {
        log::info!(
            "read_project_markdown_file path={} file={}",
            path,
            relative_path
        );
        markdown::read_markdown_file(&path, &relative_path)
    })
}

#[tauri::command]
fn collect_git_daily(paths: Vec<String>, identities: Vec<GitIdentity>) -> Vec<GitDailyResult> {
    log_command("collect_git_daily", || {
        log::info!("collect_git_daily paths={}", paths.len());
        git_daily::collect_git_daily(&paths, &identities)
    })
}

#[tauri::command]
fn load_heatmap_cache(app: AppHandle) -> Result<HeatmapCacheFile, String> {
    log_command_result("load_heatmap_cache", || storage::load_heatmap_cache(&app))
}

#[tauri::command]
fn save_heatmap_cache(app: AppHandle, cache: HeatmapCacheFile) -> Result<(), String> {
    log_command_result("save_heatmap_cache", || {
        storage::save_heatmap_cache(&app, &cache)
    })
}

#[tauri::command]
fn get_tmux_support_status() -> TmuxSupportStatus {
    terminal::tmux_support_status()
}

#[tauri::command]
fn list_codex_sessions(app: AppHandle) -> Result<Vec<CodexSessionSummary>, String> {
    log_command_result("list_codex_sessions", || {
        if let Err(error) = codex_sessions::ensure_session_watcher(&app) {
            log::warn!("启动 Codex 会话监听失败: {}", error);
        }
        codex_sessions::list_sessions(&app)
    })
}

#[tauri::command]
fn create_terminal_session(
    app: AppHandle,
    state: State<'_, Mutex<TerminalManager>>,
    project_id: String,
    project_path: String,
    project_name: String,
) -> Result<TerminalSessionInfo, String> {
    log_command_result("create_terminal_session", || {
        log::info!(
            "create_terminal_session project_id={} project_path={} project_name={}",
            project_id,
            project_path,
            project_name
        );
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .create_session(app, &project_id, &project_path, &project_name)
    })
}

#[tauri::command]
fn close_terminal_session(
    state: State<'_, Mutex<TerminalManager>>,
    session_id: String,
) -> Result<(), String> {
    log_command_result("close_terminal_session", || {
        log::info!("close_terminal_session session_id={}", session_id);
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .close_session(&session_id)
    })
}

#[tauri::command]
fn switch_terminal_session(
    app: AppHandle,
    state: State<'_, Mutex<TerminalManager>>,
    session_id: String,
) -> Result<(), String> {
    log_command_result("switch_terminal_session", || {
        log::info!("switch_terminal_session session_id={}", session_id);
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .switch_session(app, &session_id)
    })
}

#[tauri::command]
fn list_terminal_sessions(
    state: State<'_, Mutex<TerminalManager>>,
) -> Result<Vec<TerminalSessionInfo>, String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .list_sessions()
}

#[tauri::command]
fn list_tmux_windows(
    state: State<'_, Mutex<TerminalManager>>,
    session_id: String,
) -> Result<Vec<TmuxWindowInfo>, String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .list_windows(&session_id)
}

#[tauri::command]
fn list_tmux_panes(
    state: State<'_, Mutex<TerminalManager>>,
    window_id: String,
) -> Result<Vec<TmuxPaneInfo>, String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .list_panes(&window_id)
}

#[tauri::command]
fn send_tmux_input(
    state: State<'_, Mutex<TerminalManager>>,
    pane_id: String,
    data: String,
) -> Result<(), String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .send_input(&pane_id, &data)
}

#[tauri::command]
fn split_tmux_pane(
    state: State<'_, Mutex<TerminalManager>>,
    pane_id: String,
    direction: String,
) -> Result<(), String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .split_pane(&pane_id, &direction)
}

#[tauri::command]
fn select_tmux_pane(
    state: State<'_, Mutex<TerminalManager>>,
    pane_id: String,
) -> Result<(), String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .select_pane(&pane_id)
}

#[tauri::command]
fn select_tmux_pane_direction(
    state: State<'_, Mutex<TerminalManager>>,
    pane_id: String,
    direction: String,
) -> Result<(), String> {
    log_command_result("select_tmux_pane_direction", || {
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .select_pane_direction(&pane_id, &direction)
    })
}

#[tauri::command]
fn resize_tmux_pane(
    state: State<'_, Mutex<TerminalManager>>,
    pane_id: String,
    direction: String,
    count: u16,
) -> Result<(), String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .resize_pane(&pane_id, &direction, count)
}

#[tauri::command]
fn kill_tmux_pane(state: State<'_, Mutex<TerminalManager>>, pane_id: String) -> Result<(), String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .kill_pane(&pane_id)
}

#[tauri::command]
fn new_tmux_window(
    state: State<'_, Mutex<TerminalManager>>,
    session_id: String,
    project_path: String,
) -> Result<(), String> {
    log_command_result("new_tmux_window", || {
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .new_window(&session_id, &project_path)
    })
}

#[tauri::command]
fn select_tmux_window(
    state: State<'_, Mutex<TerminalManager>>,
    window_id: String,
) -> Result<(), String> {
    log_command_result("select_tmux_window", || {
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .select_window(&window_id)
    })
}

#[tauri::command]
fn select_tmux_window_index(
    state: State<'_, Mutex<TerminalManager>>,
    session_id: String,
    window_index: i32,
) -> Result<(), String> {
    log_command_result("select_tmux_window_index", || {
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .select_window_index(&session_id, window_index)
    })
}

#[tauri::command]
fn next_tmux_window(state: State<'_, Mutex<TerminalManager>>) -> Result<(), String> {
    log_command_result("next_tmux_window", || {
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .next_window()
    })
}

#[tauri::command]
fn previous_tmux_window(state: State<'_, Mutex<TerminalManager>>) -> Result<(), String> {
    log_command_result("previous_tmux_window", || {
        state
            .lock()
            .map_err(|_| "终端状态锁异常".to_string())?
            .previous_window()
    })
}

#[tauri::command]
fn resize_tmux_client(
    app: AppHandle,
    state: State<'_, Mutex<TerminalManager>>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .resize_client(app, &session_id, cols, rows)
}

#[tauri::command]
fn capture_tmux_pane(
    state: State<'_, Mutex<TerminalManager>>,
    pane_id: String,
    lines: Option<u16>,
) -> Result<String, String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .capture_pane(&pane_id, lines)
}

#[tauri::command]
fn get_tmux_pane_cursor(
    state: State<'_, Mutex<TerminalManager>>,
    pane_id: String,
) -> Result<TmuxPaneCursor, String> {
    state
        .lock()
        .map_err(|_| "终端状态锁异常".to_string())?
        .get_pane_cursor(&pane_id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// 启动 Tauri 应用。
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(Mutex::new(TerminalManager::new()))
        .setup(|app| {
            log::info!(
                "app start name={} version={}",
                app.package_info().name,
                app.package_info().version
            );
            if let Ok(path) = app.path().app_log_dir() {
                log::info!("log dir={}", path.display());
            }
            let app_handle = app.handle();
            if let Err(error) = codex_sessions::ensure_session_watcher(&app_handle) {
                log::warn!("启动 Codex 会话监听失败: {}", error);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_app_state,
            save_app_state,
            load_projects,
            save_projects,
            discover_projects,
            build_projects,
            list_branches,
            open_in_finder,
            open_in_terminal,
            open_in_editor,
            list_dev_tool_presets,
            set_window_fullscreen_auxiliary,
            copy_to_clipboard,
            read_project_notes,
            write_project_notes,
            list_project_markdown_files,
            read_project_markdown_file,
            collect_git_daily,
            load_heatmap_cache,
            save_heatmap_cache,
            get_tmux_support_status,
            list_codex_sessions,
            create_terminal_session,
            close_terminal_session,
            switch_terminal_session,
            list_terminal_sessions,
            list_tmux_windows,
            list_tmux_panes,
            send_tmux_input,
            split_tmux_pane,
            select_tmux_pane,
            select_tmux_pane_direction,
            resize_tmux_pane,
            kill_tmux_pane,
            new_tmux_window,
            select_tmux_window,
            select_tmux_window_index,
            next_tmux_window,
            previous_tmux_window,
            resize_tmux_client,
            capture_tmux_pane,
            get_tmux_pane_cursor,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "macos")]
fn apply_fullscreen_auxiliary(window: &tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{
        NSNormalWindowLevel, NSPanel, NSScreenSaverWindowLevel, NSWindow,
        NSWindowCollectionBehavior, NSWindowStyleMask,
    };

    let ns_window = window.ns_window().map_err(|error| error.to_string())?;
    if ns_window.is_null() {
        return Err("获取 NSWindow 失败".to_string());
    }

    unsafe {
        let ns_window = &*(ns_window as *mut NSWindow);
        let ns_window_obj = &*(ns_window as *const NSWindow as *const AnyObject);
        let mut behavior = ns_window.collectionBehavior();
        if enabled {
            if let Err(error) = try_set_window_class(ns_window_obj, "NSPanel") {
                log::warn!("升级为 NSPanel 失败: {}", error);
            }
            if ns_window_obj.class().name().to_bytes() == b"NSPanel" {
                let panel = &*(ns_window as *const NSWindow as *const NSPanel);
                panel.setFloatingPanel(true);
                panel.setBecomesKeyOnlyIfNeeded(true);
                panel.setWorksWhenModal(true);
            }
            let mut style = ns_window.styleMask();
            style |= NSWindowStyleMask::NonactivatingPanel;
            style |= NSWindowStyleMask::UtilityWindow;
            ns_window.setStyleMask(style);
            behavior |= NSWindowCollectionBehavior::Auxiliary;
            behavior |= NSWindowCollectionBehavior::CanJoinAllSpaces;
            behavior |= NSWindowCollectionBehavior::CanJoinAllApplications;
            behavior |= NSWindowCollectionBehavior::FullScreenAuxiliary;
            ns_window.setHidesOnDeactivate(false);
            ns_window.setLevel(NSScreenSaverWindowLevel);
            ns_window.orderFrontRegardless();
        } else {
            let mut style = ns_window.styleMask();
            style &= !NSWindowStyleMask::NonactivatingPanel;
            style &= !NSWindowStyleMask::UtilityWindow;
            ns_window.setStyleMask(style);
            behavior &= !NSWindowCollectionBehavior::FullScreenAuxiliary;
            behavior &= !NSWindowCollectionBehavior::CanJoinAllApplications;
            behavior &= !NSWindowCollectionBehavior::Auxiliary;
            ns_window.setLevel(NSNormalWindowLevel);
            if let Err(error) = try_set_window_class(ns_window_obj, "NSWindow") {
                log::warn!("还原 NSWindow 失败: {}", error);
            }
        }
        ns_window.setCollectionBehavior(behavior);
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn try_set_window_class(
    target: &objc2::runtime::AnyObject,
    class_name: &str,
) -> Result<(), String> {
    use std::ffi::CStr;

    let class_cstr = match class_name {
        "NSPanel" => CStr::from_bytes_with_nul(b"NSPanel\0").map_err(|_| "类名非法".to_string())?,
        "NSWindow" => {
            CStr::from_bytes_with_nul(b"NSWindow\0").map_err(|_| "类名非法".to_string())?
        }
        _ => return Err("不支持的类名".to_string()),
    };
    let target_class =
        objc2::runtime::AnyClass::get(class_cstr).ok_or_else(|| "无法获取目标类".to_string())?;
    let current_class = target.class();
    if current_class.name() == target_class.name() {
        return Ok(());
    }
    if current_class.instance_size() != target_class.instance_size() {
        return Err(format!(
            "类大小不匹配: {} -> {}",
            current_class.instance_size(),
            target_class.instance_size()
        ));
    }
    unsafe {
        objc2::runtime::AnyObject::set_class(target, target_class);
    }
    Ok(())
}

fn log_command<T, F: FnOnce() -> T>(name: &str, action: F) -> T {
    let start = Instant::now();
    log::info!("command {} start", name);
    let result = action();
    log::info!("command {} done {}ms", name, start.elapsed().as_millis());
    result
}

fn log_command_result<T, E: std::fmt::Display, F: FnOnce() -> Result<T, E>>(
    name: &str,
    action: F,
) -> Result<T, E> {
    let start = Instant::now();
    log::info!("command {} start", name);
    let result = action();
    match &result {
        Ok(_) => log::info!("command {} ok {}ms", name, start.elapsed().as_millis()),
        Err(err) => log::error!(
            "command {} failed {}ms: {}",
            name,
            start.elapsed().as_millis(),
            err
        ),
    }
    result
}

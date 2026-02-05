use std::path::Path;
use std::process::{Command, ExitStatus};

#[cfg(target_os = "windows")]
use std::fs;
#[cfg(target_os = "macos")]
use std::io::Write;
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Stdio;

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::models::DevToolPreset;

#[derive(Debug, serde::Deserialize)]
pub struct EditorOpenParams {
    pub path: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub command_path: Option<String>,
    pub arguments: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct TerminalOpenParams {
    pub path: String,
    pub command_path: Option<String>,
    pub arguments: Option<Vec<String>>,
}

/// 在系统文件管理器中定位路径。
pub fn open_in_finder(path: &str) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        let status = Command::new("/usr/bin/open")
            .args(["-R", path])
            .status()
            .map_err(|err| format!("无法打开 Finder: {err}"))?;
        if status.success() {
            return Ok(());
        }
        return Err("Finder 打开失败".to_string());
    }

    open_with_default(path)
}

/// 在终端中打开指定目录。
pub fn open_in_terminal(params: TerminalOpenParams) -> Result<(), String> {
    if let Some(command_path) = params.command_path {
        let arguments = build_command_arguments(params.arguments, &params.path);
        return run_command_with_shell_support(
            &command_path,
            &arguments,
            "无法打开终端:",
            "终端打开失败",
        );
    }

    if cfg!(target_os = "windows") {
        return open_windows_terminal(&params.path);
    }

    if cfg!(target_os = "macos") {
        let escaped_path = params.path.replace('"', "\\\"");
        let script = format!(
            "tell application \"Terminal\"\n    do script \"cd \\\"{}\\\"\"\n    activate\nend tell",
            escaped_path
        );
        let status = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .status()
            .map_err(|err| format!("无法打开终端: {err}"))?;
        if status.success() {
            return Ok(());
        }
        return Err("终端打开失败".to_string());
    }

    open_with_default(&params.path)
}

/// 使用指定编辑器打开文件或目录。
pub fn open_in_editor(params: EditorOpenParams) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        if let Some(app_name) = params.app_name.clone() {
            let status = Command::new("/usr/bin/open")
                .args(["-a", app_name.as_str(), params.path.as_str()])
                .status()
                .map_err(|err| format!("打开编辑器失败: {err}"))?;
            if status.success() {
                return Ok(());
            }
        }

        if let Some(bundle_id) = params.bundle_id.clone() {
            let status = Command::new("/usr/bin/open")
                .args(["-b", bundle_id.as_str(), params.path.as_str()])
                .status()
                .map_err(|err| format!("打开编辑器失败: {err}"))?;
            if status.success() {
                return Ok(());
            }
        }
    }

    if let Some(command_path) = params.command_path {
        let arguments = build_command_arguments(params.arguments, &params.path);
        return run_command_with_shell_support(
            &command_path,
            &arguments,
            "打开编辑器失败:",
            "打开编辑器失败",
        );
    }

    Err("未能打开编辑器".to_string())
}

/// 列出已检测到的开发工具预设。
pub fn list_dev_tool_presets() -> Vec<DevToolPreset> {
    #[cfg(target_os = "macos")]
    {
        return list_dev_tool_presets_macos();
    }
    #[cfg(target_os = "windows")]
    {
        return list_dev_tool_presets_windows();
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        return list_dev_tool_presets_linux();
    }
}

fn build_command_arguments(arguments: Option<Vec<String>>, path: &str) -> Vec<String> {
    let mut resolved = Vec::new();
    let mut inserted_path = false;

    if let Some(arguments) = arguments {
        for argument in arguments {
            if argument.contains("{path}") {
                resolved.push(argument.replace("{path}", path));
                inserted_path = true;
            } else {
                resolved.push(argument);
            }
        }
    }

    if !inserted_path {
        resolved.push(path.to_string());
    }

    resolved
}

fn run_command_with_shell_support(
    command_path: &str,
    arguments: &[String],
    spawn_error_prefix: &str,
    failure_message: &str,
) -> Result<(), String> {
    let status = spawn_command_with_shell_support(command_path, arguments)
        .map_err(|err| format!("{spawn_error_prefix} {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(failure_message.to_string())
    }
}

#[cfg(target_os = "windows")]
fn spawn_command_with_shell_support(
    command_path: &str,
    arguments: &[String],
) -> Result<ExitStatus, std::io::Error> {
    if let Some(kind) = resolve_windows_command_kind(command_path) {
        return execute_windows_command(kind, command_path, arguments);
    }

    match Command::new(command_path).args(arguments).status() {
        Ok(status) => Ok(status),
        Err(error) => {
            if let Some((kind, fallback_path)) =
                resolve_windows_command_fallback(command_path, &error)
            {
                execute_windows_command(kind, &fallback_path, arguments)
            } else {
                Err(error)
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn spawn_command_with_shell_support(
    command_path: &str,
    arguments: &[String],
) -> Result<ExitStatus, std::io::Error> {
    Command::new(command_path).args(arguments).status()
}

#[cfg(target_os = "windows")]
fn resolve_windows_command_kind(command_path: &str) -> Option<WindowsCommandKind> {
    let extension = Path::new(command_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())?;
    match extension.as_str() {
        "cmd" | "bat" => Some(WindowsCommandKind::Cmd),
        "ps1" => Some(WindowsCommandKind::PowerShell),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn resolve_windows_command_fallback(
    command_path: &str,
    error: &std::io::Error,
) -> Option<(WindowsCommandKind, String)> {
    if Path::new(command_path).extension().is_some() || !should_try_windows_fallback(error) {
        return None;
    }
    let base_path = Path::new(command_path);
    for (extension, kind) in [
        ("cmd", WindowsCommandKind::Cmd),
        ("bat", WindowsCommandKind::Cmd),
        ("ps1", WindowsCommandKind::PowerShell),
        ("exe", WindowsCommandKind::Direct),
        ("com", WindowsCommandKind::Direct),
    ] {
        let candidate = base_path.with_extension(extension);
        if candidate.is_file() {
            return Some((kind, candidate.to_string_lossy().to_string()));
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn should_try_windows_fallback(error: &std::io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(2) | Some(3) | Some(193) | Some(216)
    )
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
enum WindowsCommandKind {
    Direct,
    Cmd,
    PowerShell,
}

#[cfg(target_os = "windows")]
fn execute_windows_command(
    kind: WindowsCommandKind,
    executable: &str,
    arguments: &[String],
) -> Result<ExitStatus, std::io::Error> {
    match kind {
        WindowsCommandKind::Direct => Command::new(executable).args(arguments).status(),
        WindowsCommandKind::Cmd => Command::new("cmd.exe")
            .arg("/C")
            .arg(executable)
            .args(arguments)
            .status(),
        WindowsCommandKind::PowerShell => Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(executable)
            .args(arguments)
            .status(),
    }
}

/// 复制文本到系统剪贴板（跨平台）。
pub fn copy_to_clipboard(app: &AppHandle, content: &str) -> Result<(), String> {
    if let Err(err) = app.clipboard().write_text(content.to_string()) {
        #[cfg(target_os = "macos")]
        {
            let _ = err;
            return copy_with_pbcopy(content).map_err(|err| format!("写入剪贴板失败: {err}"));
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Err(format!("写入剪贴板失败: {err}"));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn copy_with_pbcopy(content: &str) -> Result<(), std::io::Error> {
    let mut child = Command::new("/usr/bin/pbcopy")
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(content.as_bytes())?;
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "写入剪贴板失败",
        ))
    }
}

// 使用系统默认方式打开路径。
#[cfg(target_os = "macos")]
fn open_with_default(path: &str) -> Result<(), String> {
    let status = Command::new("/usr/bin/open")
        .arg(path)
        .status()
        .map_err(|err| format!("无法打开路径: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("打开路径失败".to_string())
    }
}

#[cfg(target_os = "macos")]
fn list_dev_tool_presets_macos() -> Vec<DevToolPreset> {
    let mut presets = Vec::new();
    push_macos_app(
        &mut presets,
        "vscode",
        "Visual Studio Code",
        "Visual Studio Code",
    );
    push_macos_app(
        &mut presets,
        "vscode-insiders",
        "Visual Studio Code - Insiders",
        "Visual Studio Code - Insiders",
    );

    if !push_macos_app(
        &mut presets,
        "intellij-idea",
        "IntelliJ IDEA",
        "IntelliJ IDEA",
    ) {
        push_macos_app(
            &mut presets,
            "intellij-idea",
            "IntelliJ IDEA Community",
            "IntelliJ IDEA CE",
        );
    }

    if !push_macos_app(&mut presets, "pycharm", "PyCharm", "PyCharm") {
        push_macos_app(&mut presets, "pycharm", "PyCharm Community", "PyCharm CE");
    }

    push_macos_app(&mut presets, "webstorm", "WebStorm", "WebStorm");
    push_macos_app(&mut presets, "goland", "GoLand", "GoLand");
    push_macos_app(&mut presets, "rider", "Rider", "Rider");
    push_macos_app(&mut presets, "clion", "CLion", "CLion");
    push_macos_app(&mut presets, "phpstorm", "PhpStorm", "PhpStorm");
    push_macos_app(&mut presets, "datagrip", "DataGrip", "DataGrip");

    presets
}

#[cfg(target_os = "macos")]
fn push_macos_app(
    presets: &mut Vec<DevToolPreset>,
    id: &str,
    display_name: &str,
    app_name: &str,
) -> bool {
    let bundle_path = Path::new("/Applications").join(format!("{app_name}.app"));
    if !bundle_path.exists() {
        return false;
    }
    presets.push(DevToolPreset {
        id: id.to_string(),
        name: display_name.to_string(),
        command_path: "/usr/bin/open".to_string(),
        arguments: vec!["-a".to_string(), app_name.to_string(), "{path}".to_string()],
    });
    true
}

#[cfg(target_os = "windows")]
fn list_dev_tool_presets_windows() -> Vec<DevToolPreset> {
    let mut presets = Vec::new();

    if let Some(path) = find_windows_vscode() {
        presets.push(build_windows_preset("vscode", "Visual Studio Code", path));
    }
    if let Some(path) = find_windows_vscode_insiders() {
        presets.push(build_windows_preset(
            "vscode-insiders",
            "Visual Studio Code - Insiders",
            path,
        ));
    }

    if let Some(path) = find_jetbrains_toolbox_exe("IDEA-U", "idea64.exe")
        .or_else(|| find_jetbrains_toolbox_exe("IDEA-C", "idea64.exe"))
        .or_else(|| find_jetbrains_install_exe("idea64.exe"))
    {
        let name = if path.to_string_lossy().to_lowercase().contains("idea-c") {
            "IntelliJ IDEA Community"
        } else {
            "IntelliJ IDEA"
        };
        presets.push(build_windows_preset("intellij-idea", name, path));
    }

    if let Some(path) = find_jetbrains_toolbox_exe("PyCharm-P", "pycharm64.exe")
        .or_else(|| find_jetbrains_toolbox_exe("PyCharm-C", "pycharm64.exe"))
        .or_else(|| find_jetbrains_install_exe("pycharm64.exe"))
    {
        let name = if path.to_string_lossy().to_lowercase().contains("pycharm-c") {
            "PyCharm Community"
        } else {
            "PyCharm"
        };
        presets.push(build_windows_preset("pycharm", name, path));
    }

    add_jetbrains_windows_preset(
        &mut presets,
        "webstorm",
        "WebStorm",
        "WebStorm",
        "webstorm64.exe",
    );
    add_jetbrains_windows_preset(&mut presets, "goland", "GoLand", "GoLand", "goland64.exe");
    add_jetbrains_windows_preset(&mut presets, "rider", "Rider", "Rider", "rider64.exe");
    add_jetbrains_windows_preset(&mut presets, "clion", "CLion", "CLion", "clion64.exe");
    add_jetbrains_windows_preset(
        &mut presets,
        "phpstorm",
        "PhpStorm",
        "PhpStorm",
        "phpstorm64.exe",
    );
    add_jetbrains_windows_preset(
        &mut presets,
        "datagrip",
        "DataGrip",
        "DataGrip",
        "datagrip64.exe",
    );

    presets
}

#[cfg(target_os = "windows")]
fn add_jetbrains_windows_preset(
    presets: &mut Vec<DevToolPreset>,
    id: &str,
    name: &str,
    toolbox_code: &str,
    exe_name: &str,
) {
    if let Some(path) = find_jetbrains_toolbox_exe(toolbox_code, exe_name)
        .or_else(|| find_jetbrains_install_exe(exe_name))
    {
        presets.push(build_windows_preset(id, name, path));
    }
}

#[cfg(target_os = "windows")]
fn build_windows_preset(id: &str, name: &str, command_path: PathBuf) -> DevToolPreset {
    DevToolPreset {
        id: id.to_string(),
        name: name.to_string(),
        command_path: command_path.to_string_lossy().to_string(),
        arguments: vec!["{path}".to_string()],
    }
}

#[cfg(target_os = "windows")]
fn find_windows_vscode() -> Option<PathBuf> {
    find_windows_path(
        &["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"],
        &[
            PathBuf::from("Microsoft VS Code\\Code.exe"),
            PathBuf::from("Programs\\Microsoft VS Code\\Code.exe"),
        ],
    )
    .or_else(|| find_in_path("code").map(PathBuf::from))
}

#[cfg(target_os = "windows")]
fn find_windows_vscode_insiders() -> Option<PathBuf> {
    find_windows_path(
        &["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"],
        &[
            PathBuf::from("Microsoft VS Code Insiders\\Code - Insiders.exe"),
            PathBuf::from("Programs\\Microsoft VS Code Insiders\\Code - Insiders.exe"),
        ],
    )
    .or_else(|| find_in_path("code-insiders").map(PathBuf::from))
}

#[cfg(target_os = "windows")]
fn find_windows_path(env_keys: &[&str], suffixes: &[PathBuf]) -> Option<PathBuf> {
    for key in env_keys {
        if let Ok(root) = std::env::var(key) {
            let root_path = PathBuf::from(root);
            for suffix in suffixes {
                let candidate = root_path.join(suffix);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn find_jetbrains_toolbox_exe(product_code: &str, exe_name: &str) -> Option<PathBuf> {
    let local = std::env::var("LOCALAPPDATA").ok()?;
    let base = PathBuf::from(local)
        .join("JetBrains")
        .join("Toolbox")
        .join("apps")
        .join(product_code)
        .join("ch-0");
    if !base.is_dir() {
        return None;
    }
    let mut builds: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join("bin").join(exe_name);
                if candidate.is_file() {
                    builds.push(path);
                }
            }
        }
    }
    builds.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    let latest = builds.pop()?;
    let exe_path = latest.join("bin").join(exe_name);
    if exe_path.is_file() {
        Some(exe_path)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn find_jetbrains_install_exe(exe_name: &str) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(path) = std::env::var("ProgramFiles") {
        roots.push(PathBuf::from(path).join("JetBrains"));
    }
    if let Ok(path) = std::env::var("ProgramFiles(x86)") {
        roots.push(PathBuf::from(path).join("JetBrains"));
    }
    if let Ok(path) = std::env::var("LOCALAPPDATA") {
        let local_path = PathBuf::from(&path);
        roots.push(local_path.join("JetBrains"));
        roots.push(local_path.join("Programs").join("JetBrains"));
    }
    for root in roots {
        if let Some(found) = find_jetbrains_in_root(&root, exe_name) {
            return Some(found);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn find_jetbrains_in_root(root: &Path, exe_name: &str) -> Option<PathBuf> {
    if !root.is_dir() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let candidate = path.join("bin").join(exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn list_dev_tool_presets_linux() -> Vec<DevToolPreset> {
    let mut presets = Vec::new();
    if let Some(command) = find_in_path("code") {
        presets.push(build_linux_preset("vscode", "Visual Studio Code", command));
    }
    if let Some(command) = find_in_path("code-insiders") {
        presets.push(build_linux_preset(
            "vscode-insiders",
            "Visual Studio Code - Insiders",
            command,
        ));
    }

    add_linux_preset(&mut presets, "intellij-idea", "IntelliJ IDEA", "idea");
    add_linux_preset(&mut presets, "webstorm", "WebStorm", "webstorm");
    add_linux_preset(&mut presets, "pycharm", "PyCharm", "pycharm");
    add_linux_preset(&mut presets, "goland", "GoLand", "goland");
    add_linux_preset(&mut presets, "rider", "Rider", "rider");
    add_linux_preset(&mut presets, "clion", "CLion", "clion");
    add_linux_preset(&mut presets, "phpstorm", "PhpStorm", "phpstorm");
    add_linux_preset(&mut presets, "datagrip", "DataGrip", "datagrip");

    presets
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn add_linux_preset(presets: &mut Vec<DevToolPreset>, id: &str, name: &str, command: &str) {
    if let Some(command_path) = find_in_path(command) {
        presets.push(build_linux_preset(id, name, command_path));
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn build_linux_preset(id: &str, name: &str, command_path: String) -> DevToolPreset {
    DevToolPreset {
        id: id.to_string(),
        name: name.to_string(),
        command_path,
        arguments: vec!["{path}".to_string()],
    }
}

fn find_in_path(command: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;
    let has_extension = Path::new(command).extension().is_some();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
        #[cfg(target_os = "windows")]
        {
            if !has_extension {
                for ext in ["exe", "cmd", "bat"] {
                    let with_ext = dir.join(format!("{command}.{ext}"));
                    if with_ext.is_file() {
                        return Some(with_ext.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn open_with_default(path: &str) -> Result<(), String> {
    let status = Command::new("explorer")
        .arg(path)
        .status()
        .map_err(|err| format!("无法打开路径: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("打开路径失败".to_string())
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn open_with_default(path: &str) -> Result<(), String> {
    let status = Command::new("xdg-open")
        .arg(path)
        .status()
        .map_err(|err| format!("无法打开路径: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("打开路径失败".to_string())
    }
}

#[cfg(target_os = "windows")]
fn open_windows_terminal(path: &str) -> Result<(), String> {
    let wt_status = Command::new("wt.exe").arg("-d").arg(path).status();
    if let Ok(status) = wt_status {
        if status.success() {
            return Ok(());
        }
    }

    let escaped_path = path.replace('"', "\"\"");
    let command = format!("Set-Location -LiteralPath \"{}\"", escaped_path);
    let status = Command::new("powershell.exe")
        .args(["-NoExit", "-Command", command.as_str()])
        .status()
        .map_err(|err| format!("无法打开终端: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("终端打开失败".to_string())
    }
}

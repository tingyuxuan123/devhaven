use std::io::Write;
use std::process::{Command, Stdio};

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

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
        let status = Command::new(command_path)
            .args(arguments)
            .status()
            .map_err(|err| format!("无法打开终端: {err}"))?;
        if status.success() {
            return Ok(());
        }
        return Err("终端打开失败".to_string());
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
        let mut command = Command::new(command_path);
        if let Some(arguments) = params.arguments {
            command.args(arguments);
        }
        let status = command
            .arg(params.path)
            .status()
            .map_err(|err| format!("打开编辑器失败: {err}"))?;
        if status.success() {
            return Ok(());
        }
    }

    Err("未能打开编辑器".to_string())
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
    let mut child = Command::new("/usr/bin/pbcopy").stdin(Stdio::piped()).spawn()?;
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

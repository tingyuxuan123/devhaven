import { invoke } from "@tauri-apps/api/core";
import type { DevToolPreset } from "../models/types";

export type TerminalOpenParams = {
  path: string;
  command_path?: string | null;
  arguments?: string[] | null;
};

export type EditorOpenParams = {
  path: string;
  command_path?: string | null;
  arguments?: string[] | null;
};

/** 在系统文件管理器中定位路径。 */
export async function openInFinder(path: string) {
  await invoke("open_in_finder", { path });
}

/** 在终端中打开指定目录。 */
export async function openInTerminal(params: TerminalOpenParams) {
  await invoke("open_in_terminal", { params });
}

/** 使用外部编辑器打开指定目录。 */
export async function openInEditor(params: EditorOpenParams) {
  await invoke("open_in_editor", { params });
}

/** 列出已检测到的开发工具预设。 */
export async function listDevToolPresets(): Promise<DevToolPreset[]> {
  return invoke<DevToolPreset[]>("list_dev_tool_presets");
}

/** 将内容写入系统剪贴板。 */
export async function copyToClipboard(content: string) {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(content);
      return;
    } catch (error) {
      console.warn("浏览器剪贴板写入失败，尝试使用系统命令。", error);
    }
  }
  await invoke("copy_to_clipboard", { content });
}

/** 发送系统通知。 */
export async function sendSystemNotification(title: string, body?: string) {
  if (typeof window === "undefined" || typeof Notification === "undefined") {
    return;
  }
  try {
    let permission = Notification.permission;
    if (permission === "default") {
      permission = await Notification.requestPermission();
    }
    if (permission !== "granted") {
      return;
    }
    const options = body ? { body } : undefined;
    new Notification(title, options);
  } catch (error) {
    console.warn("系统通知发送失败。", error);
  }
}

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type SwiftDate = f64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateFile {
    pub version: i32,
    pub tags: Vec<TagData>,
    pub directories: Vec<String>,
    #[serde(default, rename = "recycleBin")]
    pub recycle_bin: Vec<String>,
    #[serde(default)]
    pub settings: AppSettings,
}

impl Default for AppStateFile {
    /// 默认应用状态结构。
    fn default() -> Self {
        Self {
            version: 5,
            tags: Vec::new(),
            directories: Vec::new(),
            recycle_bin: Vec::new(),
            settings: AppSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub editor_open_tool: OpenToolSettings,
    #[serde(default)]
    pub terminal_open_tool: OpenToolSettings,
    #[serde(default = "default_terminal_use_webgl_renderer")]
    pub terminal_use_webgl_renderer: bool,
    #[serde(default)]
    pub show_monitor_window: bool,
    #[serde(default = "default_workspace_enabled")]
    pub workspace_enabled: bool,
    #[serde(default)]
    pub git_identities: Vec<GitIdentity>,
    #[serde(default)]
    pub dev_tools: Vec<DevToolConfig>,
    #[serde(default)]
    pub default_dev_tool_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            editor_open_tool: OpenToolSettings::default(),
            terminal_open_tool: OpenToolSettings::default(),
            terminal_use_webgl_renderer: true,
            show_monitor_window: false,
            workspace_enabled: default_workspace_enabled(),
            git_identities: Vec::new(),
            dev_tools: Vec::new(),
            default_dev_tool_id: String::new(),
        }
    }
}

fn default_terminal_use_webgl_renderer() -> bool {
    true
}

fn default_workspace_enabled() -> bool {
    cfg!(target_os = "macos")
}

fn default_dev_tool_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenToolSettings {
    pub command_path: String,
    pub arguments: Vec<String>,
}

impl Default for OpenToolSettings {
    fn default() -> Self {
        Self {
            command_path: String::new(),
            arguments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitIdentity {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevToolConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub command_path: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default = "default_dev_tool_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub is_preset: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevToolPreset {
    pub id: String,
    pub name: String,
    pub command_path: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagData {
    pub name: String,
    pub color: ColorData,
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorData {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub tags: Vec<String>,
    pub mtime: SwiftDate,
    pub size: i64,
    pub checksum: String,
    pub git_commits: i64,
    pub git_last_commit: SwiftDate,
    pub git_daily: Option<String>,
    pub created: SwiftDate,
    pub checked: SwiftDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownFileEntry {
    pub path: String,
    pub absolute_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDailyResult {
    pub path: String,
    #[serde(rename = "gitDaily")]
    pub git_daily: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapCacheEntry {
    #[serde(rename = "dateString")]
    pub date_string: String,
    #[serde(rename = "commitCount")]
    pub commit_count: i64,
    #[serde(rename = "projectIds")]
    pub project_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapCacheFile {
    pub version: i32,
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
    #[serde(rename = "dailyActivity")]
    pub daily_activity: HashMap<String, HeatmapCacheEntry>,
    #[serde(rename = "projectCount")]
    pub project_count: i64,
    #[serde(default, rename = "gitDailySignature")]
    pub git_daily_signature: String,
}

impl Default for HeatmapCacheFile {
    fn default() -> Self {
        Self {
            version: 1,
            last_updated: String::new(),
            daily_activity: HashMap::new(),
            project_count: 0,
            git_daily_signature: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchListItem {
    pub name: String,
    #[serde(rename = "isMain")]
    pub is_main: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMessageCounts {
    pub user: i32,
    pub agent: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexLastEventType {
    User,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSessionSummary {
    pub id: String,
    pub cwd: String,
    pub cli_version: Option<String>,
    pub started_at: i64,
    pub last_activity_at: i64,
    pub is_running: bool,
    pub last_user_message: Option<String>,
    pub last_agent_message: Option<String>,
    pub message_counts: CodexMessageCounts,
    pub last_event_type: Option<CodexLastEventType>,
}

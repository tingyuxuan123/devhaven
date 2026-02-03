import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import "./App.css";
import Sidebar from "./components/Sidebar";
import MainContent from "./components/MainContent";
import DetailPanel from "./components/DetailPanel";
import TagEditDialog from "./components/TagEditDialog";
import DashboardModal from "./components/DashboardModal";
import SettingsModal from "./components/SettingsModal";
import WorkspaceView from "./components/WorkspaceView";
import RecycleBinModal from "./components/RecycleBinModal";
import MonitorWindow from "./components/MonitorWindow";
import { useCodexSessions } from "./hooks/useCodexSessions";
import type { DateFilter, GitFilter } from "./models/filters";
import { DATE_FILTER_OPTIONS } from "./models/filters";
import type { HeatmapData } from "./models/heatmap";
import { HEATMAP_CONFIG } from "./models/heatmap";
import type { TerminalSessionInfo, TmuxSupportStatus, WorkspaceSession } from "./models/terminal";
import type { CodexSessionSummary, CodexSessionView } from "./models/codex";
import type { ColorData, DevTool, DevToolPreset, Project, TagData } from "./models/types";
import { swiftDateToJsDate } from "./models/types";
import { colorDataToHex } from "./utils/colors";
import { mergeDevTools } from "./utils/devTools";
import { formatDateKey } from "./utils/gitDaily";
import { buildGitIdentitySignature } from "./utils/gitIdentity";
import { pickColorForTag } from "./utils/tagColors";
import { DevHavenProvider, useDevHavenContext } from "./state/DevHavenContext";
import { useHeatmapData } from "./state/useHeatmapData";
import {
  copyToClipboard,
  listDevToolPresets,
  openInEditor,
  openInTerminal,
  sendSystemNotification,
} from "./services/system";
import { closeMonitorWindow, openMonitorWindow } from "./services/monitorWindow";
import {
  closeTerminalSession,
  createTerminalSession,
  getTmuxSupportStatus,
  listTerminalSessions,
} from "./services/terminal";

type AppMode = "gallery" | "workspace";

const MONITOR_OPEN_SESSION_EVENT = "monitor-open-session";
const MAIN_WINDOW_LABEL = "main";

type MonitorOpenSessionPayload = {
  sessionId: string;
  projectId: string | null;
  projectPath: string | null;
  projectName: string | null;
  cwd: string;
};

function matchProjectByCwd(cwd: string, projects: Project[]): Project | null {
  if (!cwd) {
    return null;
  }
  let bestMatch: Project | null = null;
  let bestLength = -1;
  for (const project of projects) {
    if (cwd.startsWith(project.path) && project.path.length > bestLength) {
      bestMatch = project;
      bestLength = project.path.length;
    }
  }
  return bestMatch;
}

function resolveAppView(): "main" | "monitor" {
  if (typeof window === "undefined") {
    return "main";
  }
  const params = new URLSearchParams(window.location.search);
  if (params.get("view") === "monitor") {
    return "monitor";
  }
  return "main";
}

function buildCodexSessionViews(sessions: CodexSessionSummary[], projects: Project[]): CodexSessionView[] {
  return sessions.map((session) => {
    const project = matchProjectByCwd(session.cwd, projects);
    return {
      ...session,
      projectId: project?.id ?? null,
      projectName: project?.name ?? null,
      projectPath: project?.path ?? null,
    };
  });
}

/** 应用主布局，负责筛选、状态联动与面板展示。 */
function AppLayout() {
  const {
    appState,
    projects,
    projectMap,
    isLoading,
    error,
    refresh,
    addDirectory,
    removeDirectory,
    addProjects,
    addTag,
    renameTag,
    removeTag,
    toggleTagHidden,
    setTagColor,
    addTagToProject,
    removeTagFromProject,
    refreshProject,
    updateGitDaily,
    updateSettings,
    moveProjectToRecycleBin,
    restoreProjectFromRecycleBin,
  } = useDevHavenContext();

  const [searchText, setSearchText] = useState("");
  const [dateFilter, setDateFilter] = useState<DateFilter>("all");
  const [gitFilter, setGitFilter] = useState<GitFilter>("all");
  const [selectedTags, setSelectedTags] = useState<Set<string>>(new Set());
  const [selectedDirectory, setSelectedDirectory] = useState<string | null>(null);
  const [selectedProjects, setSelectedProjects] = useState<Set<string>>(new Set());
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [showDetailPanel, setShowDetailPanel] = useState(false);
  const [toast, setToast] = useState<{ message: string; variant: "success" | "error" } | null>(null);
  const [heatmapFilteredProjectIds, setHeatmapFilteredProjectIds] = useState<Set<string>>(new Set());
  const [heatmapSelectedDateKey, setHeatmapSelectedDateKey] = useState<string | null>(null);
  const [tagDialogState, setTagDialogState] = useState<{ mode: "new" | "edit"; tag?: TagData } | null>(
    null,
  );
  const [showDashboard, setShowDashboard] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [devToolPresets, setDevToolPresets] = useState<DevToolPreset[]>([]);
  const [previewTerminalUseWebglRenderer, setPreviewTerminalUseWebglRenderer] = useState<boolean | null>(null);
  const [appMode, setAppMode] = useState<AppMode>("gallery");
  const [workspaceSessions, setWorkspaceSessions] = useState<WorkspaceSession[]>([]);
  const [activeWorkspaceSessionId, setActiveWorkspaceSessionId] = useState<string | null>(null);
  const [tmuxSupport, setTmuxSupport] = useState<TmuxSupportStatus>({ supported: true });
  const [tmuxSessionsLoaded, setTmuxSessionsLoaded] = useState(false);
  const [showRecycleBin, setShowRecycleBin] = useState(false);
  const appView = useMemo(() => resolveAppView(), []);
  const isMonitorView = appView === "monitor";

  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }
    document.body.classList.toggle("is-monitor-view", isMonitorView);
    document.documentElement.classList.toggle("is-monitor-view", isMonitorView);
    return () => {
      document.body.classList.remove("is-monitor-view");
      document.documentElement.classList.remove("is-monitor-view");
    };
  }, [isMonitorView]);

  useEffect(() => {
    let active = true;
    listDevToolPresets()
      .then((presets) => {
        if (active) {
          setDevToolPresets(presets);
        }
      })
      .catch((error) => {
        console.warn("读取开发工具预设失败。", error);
      });
    return () => {
      active = false;
    };
  }, []);

  const searchInputRef = useRef<HTMLInputElement>(null);
  const toastTimerRef = useRef<number | null>(null);
  const gitDailyRefreshRef = useRef<string | null>(null);
  const gitDailyUpdatingRef = useRef(false);
  const gitIdentitySignatureRef = useRef<string | null>(null);
  const pendingMonitorSessionRef = useRef<MonitorOpenSessionPayload | null>(null);
  const codexSessionSnapshotRef = useRef<Map<string, CodexSessionView>>(new Map());
  const codexSessionSnapshotReadyRef = useRef(false);
  const recycleBinPaths = appState.recycleBin ?? [];
  const recycleBinSet = useMemo(() => new Set(recycleBinPaths), [recycleBinPaths]);
  const recycleBinCount = recycleBinPaths.length;
  const visibleProjects = useMemo(
    () => projects.filter((project) => !recycleBinSet.has(project.path)),
    [projects, recycleBinSet],
  );
  const recycleBinItems = useMemo(() => {
    const projectsByPath = new Map(projects.map((project) => [project.path, project]));
    return recycleBinPaths.map((path) => {
      const project = projectsByPath.get(path);
      return {
        path,
        name: project?.name ?? path.split("/").pop() ?? path,
        missing: !project,
      };
    });
  }, [projects, recycleBinPaths]);
  const heatmapStore = useHeatmapData(visibleProjects, appState.settings.gitIdentities);
  const sidebarHeatmapData = useMemo(
    () => heatmapStore.getHeatmapData(HEATMAP_CONFIG.sidebar.days),
    [heatmapStore],
  );
  const codexSessionStore = useCodexSessions();
  const codexSessionViews = useMemo(
    () => buildCodexSessionViews(codexSessionStore.sessions, projects),
    [codexSessionStore.sessions, projects],
  );
  const runningCodexSessionViews = useMemo(
    () => codexSessionViews.filter((session) => session.isRunning),
    [codexSessionViews],
  );
  const terminalSettings = appState.settings.terminalOpenTool;
  const terminalUseWebglRenderer =
    previewTerminalUseWebglRenderer ?? appState.settings.terminalUseWebglRenderer;
  const mergedDevTools = useMemo(
    () => mergeDevTools(appState.settings.devTools, devToolPresets),
    [appState.settings.devTools, devToolPresets],
  );
  const enabledDevTools = useMemo(
    () => mergedDevTools.filter((tool) => tool.enabled),
    [mergedDevTools],
  );
  const defaultDevTool = useMemo(
    () => enabledDevTools.find((tool) => tool.id === appState.settings.defaultDevToolId) ?? null,
    [appState.settings.defaultDevToolId, enabledDevTools],
  );

  const hiddenTags = useMemo(
    () => new Set(appState.tags.filter((tag) => tag.hidden).map((tag) => tag.name)),
    [appState.tags],
  );

  const buildWorkspaceSession = useCallback(
    (sessionInfo: TerminalSessionInfo): WorkspaceSession => {
      const byPath =
        sessionInfo.projectPath.length > 0
          ? projects.find((project) => project.path === sessionInfo.projectPath)
          : null;
      const byName = projects.find((project) => project.name === sessionInfo.id);
      const byId = projectMap.get(sessionInfo.projectId) ?? null;
      const matchedProject = byPath ?? byName ?? byId;
      return {
        ...sessionInfo,
        projectId: matchedProject?.id ?? sessionInfo.projectId,
        projectPath: matchedProject?.path ?? sessionInfo.projectPath,
        projectName: sessionInfo.id,
      };
    },
    [projectMap, projects],
  );

  const filteredProjects = useMemo(() => {
    let result = [...visibleProjects];

    if (selectedDirectory) {
      result = result.filter((project) => project.path.startsWith(selectedDirectory));
    }

    result = result.filter((project) => {
      const projectHiddenTags = project.tags.filter((tag) => hiddenTags.has(tag));
      if (projectHiddenTags.length === 0) {
        return true;
      }
      if (selectedTags.size > 0 && !selectedTags.has("全部")) {
        return Array.from(selectedTags).some((tag) => projectHiddenTags.includes(tag));
      }
      return false;
    });

    if (heatmapFilteredProjectIds.size > 0) {
      result = result.filter((project) => heatmapFilteredProjectIds.has(project.id));
    } else if (selectedTags.size > 0) {
      if (!selectedTags.has("全部")) {
        const selectedTagList = Array.from(selectedTags);
        result = result.filter((project) => selectedTagList.every((tag) => project.tags.includes(tag)));
      }
    }

    const trimmedSearch = searchText.trim().toLowerCase();
    if (trimmedSearch) {
      result = result.filter(
        (project) =>
          project.name.toLowerCase().includes(trimmedSearch) || project.path.toLowerCase().includes(trimmedSearch),
      );
    }

    const dateOption = DATE_FILTER_OPTIONS.find((option) => option.value === dateFilter);
    if (dateOption?.days) {
      const cutoff = Date.now() - dateOption.days * 24 * 60 * 60 * 1000;
      result = result.filter((project) => swiftDateToJsDate(project.mtime).getTime() >= cutoff);
    }

    if (gitFilter === "gitOnly") {
      result = result.filter((project) => (project.git_commits ?? 0) > 0);
    } else if (gitFilter === "nonGitOnly") {
      result = result.filter((project) => (project.git_commits ?? 0) === 0);
    }

    result.sort((left, right) => {
      return right.mtime - left.mtime;
    });

    return result;
  }, [
    visibleProjects,
    selectedDirectory,
    selectedTags,
    heatmapFilteredProjectIds,
    searchText,
    dateFilter,
    gitFilter,
    hiddenTags,
  ]);

  const selectedProject = selectedProjectId ? projectMap.get(selectedProjectId) ?? null : null;
  const resolvedSelectedProject =
    selectedProject && recycleBinSet.has(selectedProject.path) ? null : selectedProject;

  const handleSelectTag = useCallback((tag: string) => {
    if (tag === "全部") {
      setSelectedTags(new Set());
      return;
    }
    setSelectedTags(new Set([tag]));
  }, []);

  const handleSelectProject = useCallback((project: { id: string }, event: React.MouseEvent<HTMLDivElement>) => {
    const isMulti = event.shiftKey || event.metaKey || event.ctrlKey;
    setSelectedProjects((prev) => {
      const next = new Set(prev);
      if (isMulti) {
        if (next.has(project.id)) {
          next.delete(project.id);
        } else {
          next.add(project.id);
        }
      } else {
        next.clear();
        next.add(project.id);
      }
      return next;
    });
    setSelectedProjectId(project.id);
  }, []);

  const handleSelectHeatmapDate = useCallback((entry: HeatmapData | null) => {
    if (!entry) {
      setHeatmapFilteredProjectIds(new Set());
      setHeatmapSelectedDateKey(null);
      return;
    }
    setHeatmapFilteredProjectIds(new Set(entry.projectIds));
    setHeatmapSelectedDateKey(formatDateKey(entry.date));
  }, []);

  const handleToggleDetail = useCallback(() => {
    setShowDetailPanel((prev) => {
      const next = !prev;
      if (next && !selectedProjectId && selectedProjects.size > 0) {
        setSelectedProjectId(Array.from(selectedProjects)[0]);
      }
      return next;
    });
  }, [selectedProjectId, selectedProjects]);

  const handleAssignTagToProjects = useCallback(
    async (tag: string, projectIds: string[]) => {
      await Promise.all(projectIds.map((projectId) => addTagToProject(projectId, tag)));
      setSelectedTags(new Set());
    },
    [addTagToProject],
  );

  const handleOpenTagEditor = useCallback((tag?: TagData) => {
    setTagDialogState({ mode: tag ? "edit" : "new", tag });
  }, []);

  const showToast = useCallback((message: string, variant: "success" | "error" = "success") => {
    setToast({ message, variant });
    if (toastTimerRef.current) {
      window.clearTimeout(toastTimerRef.current);
    }
    toastTimerRef.current = window.setTimeout(() => {
      setToast(null);
      toastTimerRef.current = null;
    }, 1600);
  }, []);

  const handleMoveProjectToRecycleBin = useCallback(
    async (project: Project) => {
      try {
        await moveProjectToRecycleBin(project.path);
        showToast("已移入回收站");
        setSelectedProjects((prev) => {
          if (!prev.has(project.id)) {
            return prev;
          }
          const next = new Set(prev);
          next.delete(project.id);
          return next;
        });
        setSelectedProjectId((prev) => (prev === project.id ? null : prev));
        setHeatmapFilteredProjectIds((prev) => {
          if (!prev.has(project.id)) {
            return prev;
          }
          const next = new Set(prev);
          next.delete(project.id);
          return next;
        });
      } catch (error) {
        console.error("移入回收站失败。", error);
        showToast("移入回收站失败，请稍后重试", "error");
      }
    },
    [moveProjectToRecycleBin, showToast],
  );

  const handleRestoreProjectFromRecycleBin = useCallback(
    async (path: string) => {
      try {
        await restoreProjectFromRecycleBin(path);
        showToast("已从回收站恢复");
      } catch (error) {
        console.error("恢复项目失败。", error);
        showToast("回收站恢复失败，请稍后重试", "error");
      }
    },
    [restoreProjectFromRecycleBin, showToast],
  );

  useEffect(() => {
    return () => {
      if (toastTimerRef.current) {
        window.clearTimeout(toastTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    let canceled = false;
    void (async () => {
      try {
        const status = await getTmuxSupportStatus();
        if (!canceled) {
          setTmuxSupport(status);
        }
      } catch (error) {
        console.error("获取 tmux 支持状态失败。", error);
        if (!canceled) {
          setTmuxSupport({ supported: false, reason: "无法检测 tmux 状态" });
        }
      }
    })();
    return () => {
      canceled = true;
    };
  }, []);

  useEffect(() => {
    if (!tmuxSupport.supported || isLoading || tmuxSessionsLoaded) {
      return;
    }
    let canceled = false;
    void (async () => {
      try {
        const sessions = await listTerminalSessions();
        if (!canceled) {
          setWorkspaceSessions(sessions.map((session) => buildWorkspaceSession(session)));
        }
      } catch (error) {
        console.error("加载 tmux 会话失败。", error);
      } finally {
        if (!canceled) {
          setTmuxSessionsLoaded(true);
        }
      }
    })();
    return () => {
      canceled = true;
    };
  }, [buildWorkspaceSession, isLoading, tmuxSessionsLoaded, tmuxSupport.supported]);

  useEffect(() => {
    if (isLoading) {
      return;
    }
    const missingDaily = visibleProjects.filter((project) => project.git_commits > 0 && !project.git_daily);
    if (missingDaily.length === 0) {
      gitDailyRefreshRef.current = null;
      return;
    }
    const signature = missingDaily
      .map((project) => project.path)
      .sort()
      .join("|");
    if (gitDailyUpdatingRef.current || gitDailyRefreshRef.current === signature) {
      return;
    }
    gitDailyRefreshRef.current = signature;
    gitDailyUpdatingRef.current = true;
    void (async () => {
      try {
        await updateGitDaily(missingDaily.map((project) => project.path));
      } finally {
        gitDailyUpdatingRef.current = false;
      }
    })();
  }, [isLoading, updateGitDaily, visibleProjects]);

  const gitIdentitySignature = useMemo(
    () => buildGitIdentitySignature(appState.settings.gitIdentities),
    [appState.settings.gitIdentities],
  );

  useEffect(() => {
    if (isLoading) {
      return;
    }
    if (gitIdentitySignatureRef.current === null) {
      gitIdentitySignatureRef.current = gitIdentitySignature;
      return;
    }
    if (gitIdentitySignatureRef.current === gitIdentitySignature) {
      return;
    }
    gitIdentitySignatureRef.current = gitIdentitySignature;
    const gitPaths = visibleProjects.filter((project) => project.git_commits > 0).map((project) => project.path);
    if (gitPaths.length === 0) {
      return;
    }
    void updateGitDaily(gitPaths);
  }, [gitIdentitySignature, isLoading, updateGitDaily, visibleProjects]);

  useEffect(() => {
    if (workspaceSessions.length === 0) {
      setActiveWorkspaceSessionId(null);
      if (appMode === "workspace") {
        setAppMode("gallery");
      }
      return;
    }

    if (activeWorkspaceSessionId && workspaceSessions.some((session) => session.id === activeWorkspaceSessionId)) {
      return;
    }
    setActiveWorkspaceSessionId(workspaceSessions[0].id);
  }, [activeWorkspaceSessionId, appMode, workspaceSessions]);

  const handleTagSubmit = useCallback(
    async (name: string, colorHex: string) => {
      if (!tagDialogState) {
        return;
      }
      if (tagDialogState.mode === "new") {
        await addTag(name, colorHex);
      } else if (tagDialogState.tag) {
        if (tagDialogState.tag.name !== name) {
          await renameTag(tagDialogState.tag.name, name);
        }
        await setTagColor(name, colorHex);
      }
      setTagDialogState(null);
    },
    [addTag, renameTag, setTagColor, tagDialogState],
  );

  /** 获取标签对应的显示颜色。 */
  const getTagColor = useCallback(
    (tagName: string) => {
      const tag = appState.tags.find((item) => item.name === tagName);
      if (tag) {
        return colorDataToHex(tag.color, pickColorForTag(tagName));
      }
      return pickColorForTag(tagName);
    },
    [appState.tags],
  );

  /** 将颜色结构转换为 Hex 字符串，供颜色选择器使用。 */
  const getTagHex = (color?: ColorData) => {
    if (!color) {
      return "#4d4d4d";
    }
    const toHex = (value: number) => Math.round(value * 255).toString(16).padStart(2, "0");
    return `#${toHex(color.r)}${toHex(color.g)}${toHex(color.b)}`;
  };

  const handleCopyPath = useCallback(
    async (path: string) => {
      try {
        await copyToClipboard(path);
        showToast("路径已复制");
      } catch (error) {
        console.error("复制路径失败。", error);
        showToast("复制失败，请重试", "error");
      }
    },
    [showToast],
  );

  const handleOpenInTerminal = useCallback(
    async (path: string) => {
      const commandPath = terminalSettings.commandPath.trim();
      const argumentsList = terminalSettings.arguments.map((arg) => arg.trim()).filter(Boolean);
      try {
        await openInTerminal({
          path,
          command_path: commandPath.length > 0 ? commandPath : null,
          arguments: commandPath.length > 0 && argumentsList.length > 0 ? argumentsList : null,
        });
      } catch (error) {
        console.error("终端打开失败。", error);
        showToast("终端打开失败，请检查终端配置", "error");
      }
    },
    [terminalSettings.arguments, terminalSettings.commandPath, showToast],
  );

  const handleOpenInDevTool = useCallback(
    async (tool: DevTool, path: string) => {
      const commandPath = tool.commandPath.trim();
      const argumentsList = tool.arguments.map((arg) => arg.trim()).filter(Boolean);
      if (!commandPath) {
        showToast("开发工具未配置命令", "error");
        return;
      }
      try {
        await openInEditor({
          path,
          command_path: commandPath,
          arguments: argumentsList.length > 0 ? argumentsList : null,
        });
      } catch (error) {
        console.error("开发工具打开失败。", error);
        showToast(`${tool.name} 打开失败，请检查配置`, "error");
      }
    },
    [showToast],
  );

  const handleOpenInDefaultDevTool = useCallback(
    async (path: string) => {
      if (!defaultDevTool) {
        showToast("未设置默认开发工具", "error");
        return;
      }
      await handleOpenInDevTool(defaultDevTool, path);
    },
    [defaultDevTool, handleOpenInDevTool, showToast],
  );

  const handleEnterWorkspace = useCallback(
    async (project: Project) => {
      if (!tmuxSupport.supported) {
        showToast(tmuxSupport.reason ?? "tmux 工作空间不可用", "error");
        return;
      }
      const existing = workspaceSessions.find(
        (session) =>
          session.projectId === project.id ||
          (session.projectPath.length > 0 && session.projectPath === project.path) ||
          session.projectName === project.name,
      );
      if (existing) {
        setActiveWorkspaceSessionId(existing.id);
        setAppMode("workspace");
        return;
      }

      try {
        const sessionInfo = await createTerminalSession(project.id, project.path, project.name);
        const nextSession = buildWorkspaceSession(sessionInfo);
        setWorkspaceSessions((prev) => {
          if (prev.some((session) => session.id === sessionInfo.id || session.projectId === project.id)) {
            return prev;
          }
          return [...prev, nextSession];
        });
        setActiveWorkspaceSessionId(sessionInfo.id);
        setAppMode("workspace");
      } catch (error) {
        console.error("终端会话创建失败。", error);
        showToast("终端启动失败，请检查默认 shell 与权限", "error");
      }
    },
    [buildWorkspaceSession, showToast, tmuxSupport.reason, tmuxSupport.supported, workspaceSessions],
  );

  const handleOpenCodexSession = useCallback(
    (session: CodexSessionView) => {
      if (!session.projectId) {
        showToast("未能匹配到项目", "error");
        return;
      }
      const project = projectMap.get(session.projectId);
      if (!project) {
        showToast("项目不存在或已移除", "error");
        return;
      }
      void handleEnterWorkspace(project);
    },
    [handleEnterWorkspace, projectMap, showToast],
  );

  const resolveProjectFromPayload = useCallback(
    (payload: MonitorOpenSessionPayload) =>
      (payload.projectId ? projectMap.get(payload.projectId) ?? null : null) ??
      (payload.projectPath ? projects.find((item) => item.path === payload.projectPath) ?? null : null) ??
      (payload.projectName ? projects.find((item) => item.name === payload.projectName) ?? null : null) ??
      (payload.cwd ? matchProjectByCwd(payload.cwd, projects) : null),
    [projectMap, projects],
  );

  const handleMonitorOpenCodexSession = useCallback(async (session: CodexSessionView) => {
    try {
      const mainWindow = await WebviewWindow.getByLabel(MAIN_WINDOW_LABEL);
      if (mainWindow) {
        await mainWindow.show().catch(() => undefined);
        await mainWindow.setFocus().catch(() => undefined);
      }
      await emitTo<MonitorOpenSessionPayload>(MAIN_WINDOW_LABEL, MONITOR_OPEN_SESSION_EVENT, {
        sessionId: session.id,
        projectId: session.projectId,
        projectPath: session.projectPath,
        projectName: session.projectName,
        cwd: session.cwd,
      });
    } catch (error) {
      console.error("从悬浮窗跳转项目失败。", error);
    }
  }, []);

  useEffect(() => {
    if (isMonitorView) {
      return;
    }
    let unlisten: (() => void) | null = null;
    const registerListener = async () => {
      try {
        unlisten = await listen<MonitorOpenSessionPayload>(MONITOR_OPEN_SESSION_EVENT, (event) => {
          const payload = event.payload;
          const project = resolveProjectFromPayload(payload);
          if (!project) {
            if (isLoading) {
              const pending = pendingMonitorSessionRef.current;
              if (!pending || pending.sessionId !== payload.sessionId) {
                showToast("项目加载中，稍后自动进入");
              }
              pendingMonitorSessionRef.current = payload;
              return;
            }
            showToast("项目不存在或已移除", "error");
            return;
          }
          void handleEnterWorkspace(project);
        });
      } catch (error) {
        console.error("监听悬浮窗跳转事件失败。", error);
      }
    };
    void registerListener();
    return () => {
      unlisten?.();
    };
  }, [handleEnterWorkspace, isLoading, isMonitorView, resolveProjectFromPayload, showToast]);

  useEffect(() => {
    if (isMonitorView || isLoading) {
      return;
    }
    const pending = pendingMonitorSessionRef.current;
    if (!pending) {
      return;
    }
    const project = resolveProjectFromPayload(pending);
    pendingMonitorSessionRef.current = null;
    if (!project) {
      showToast("项目不存在或已移除", "error");
      return;
    }
    void handleEnterWorkspace(project);
  }, [handleEnterWorkspace, isLoading, isMonitorView, resolveProjectFromPayload, showToast]);

  useEffect(() => {
    if (isMonitorView || codexSessionStore.isLoading) {
      return;
    }
    const previousSessions = codexSessionSnapshotRef.current;
    const nextSessions = new Map(codexSessionViews.map((session) => [session.id, session]));
    if (codexSessionSnapshotReadyRef.current) {
      for (const [sessionId, previousSession] of previousSessions) {
        if (nextSessions.has(sessionId)) {
          continue;
        }
        const lastEventType = previousSession.lastEventType;
        const isAgentCompleted =
          lastEventType === "agent" ||
          (!lastEventType &&
            previousSession.messageCounts.agent > 0 &&
            previousSession.messageCounts.agent >= previousSession.messageCounts.user &&
            Boolean(previousSession.lastAgentMessage));
        if (!isAgentCompleted) {
          continue;
        }
        const project = resolveProjectFromPayload({
          sessionId: previousSession.id,
          projectId: previousSession.projectId,
          projectPath: previousSession.projectPath,
          projectName: previousSession.projectName,
          cwd: previousSession.cwd,
        });
        if (!project) {
          showToast("Codex 已完成，但未匹配到项目", "error");
          continue;
        }
        showToast(`Codex 已完成：${project.name}`);
        void sendSystemNotification("Codex 已完成", `已跳转到 ${project.name}`);
        void handleEnterWorkspace(project);
      }
    }
    codexSessionSnapshotRef.current = nextSessions;
    codexSessionSnapshotReadyRef.current = true;
  }, [
    codexSessionStore.isLoading,
    codexSessionViews,
    handleEnterWorkspace,
    isMonitorView,
    resolveProjectFromPayload,
    showToast,
  ]);

  const handleCloseWorkspaceSession = useCallback(
    async (sessionId: string) => {
      try {
        await closeTerminalSession(sessionId);
      } catch (error) {
        console.error("终端会话关闭失败。", error);
        showToast("终端关闭失败，请稍后重试", "error");
      }
      setWorkspaceSessions((prev) => prev.filter((session) => session.id !== sessionId));
    },
    [showToast],
  );

  const handleSelectWorkspaceSession = useCallback((sessionId: string) => {
    setActiveWorkspaceSessionId(sessionId);
  }, []);

  const handleExitWorkspace = useCallback(() => {
    setAppMode("gallery");
  }, []);

  const handleCloseSettings = useCallback(() => {
    setShowSettings(false);
    setPreviewTerminalUseWebglRenderer(null);
  }, []);

  const handlePreviewTerminalRenderer = useCallback((enabled: boolean) => {
    setPreviewTerminalUseWebglRenderer(enabled);
  }, []);

  const handleSaveSettings = useCallback(
    async (settings: typeof appState.settings) => {
      try {
        await updateSettings(settings);
      } catch (error) {
        console.error("保存设置失败。", error);
        showToast("保存失败，请稍后重试", "error");
      }
    },
    [showToast, updateSettings],
  );

  useEffect(() => {
    if (isMonitorView) {
      return;
    }
    if (appState.settings.showMonitorWindow) {
      void openMonitorWindow();
    } else {
      void closeMonitorWindow();
    }
  }, [appState.settings.showMonitorWindow, isMonitorView]);

  if (isMonitorView) {
    return (
      <div className="app-root is-monitor">
        <MonitorWindow
          sessions={runningCodexSessionViews}
          isLoading={codexSessionStore.isLoading}
          error={codexSessionStore.error}
          onOpenSession={handleMonitorOpenCodexSession}
        />
      </div>
    );
  }

  return (
    <div className={`app-root${appMode === "workspace" ? " is-workspace" : ""}`}>
      {appMode === "workspace" ? (
        <WorkspaceView
          sessions={workspaceSessions}
          activeSessionId={activeWorkspaceSessionId}
          onSelectSession={handleSelectWorkspaceSession}
          onCloseSession={handleCloseWorkspaceSession}
          onExitWorkspace={handleExitWorkspace}
          terminalUseWebglRenderer={terminalUseWebglRenderer}
        />
      ) : (
        <div className={`app-split${showDetailPanel ? " has-detail" : ""}`}>
          <Sidebar
            appState={appState}
            projects={visibleProjects}
            heatmapData={sidebarHeatmapData}
            heatmapSelectedDateKey={heatmapSelectedDateKey}
            selectedTags={selectedTags}
            selectedDirectory={selectedDirectory}
            heatmapFilteredProjectIds={heatmapFilteredProjectIds}
            onSelectTag={handleSelectTag}
            onClearHeatmapFilter={() => {
              setHeatmapFilteredProjectIds(new Set());
              setHeatmapSelectedDateKey(null);
            }}
            onSelectHeatmapDate={handleSelectHeatmapDate}
            onSelectDirectory={setSelectedDirectory}
            onOpenTagEditor={handleOpenTagEditor}
            onToggleTagHidden={toggleTagHidden}
            onRemoveTag={removeTag}
            onAssignTagToProjects={handleAssignTagToProjects}
            onAddDirectory={addDirectory}
            onRemoveDirectory={removeDirectory}
            onOpenRecycleBin={() => setShowRecycleBin(true)}
            onRefresh={refresh}
            onAddProjects={addProjects}
            isHeatmapLoading={heatmapStore.isLoading}
            codexSessions={codexSessionViews}
            codexSessionsLoading={codexSessionStore.isLoading}
            codexSessionsError={codexSessionStore.error}
            onOpenCodexSession={handleOpenCodexSession}
          />
          <MainContent
            projects={visibleProjects}
            filteredProjects={filteredProjects}
            recycleBinCount={recycleBinCount}
            isLoading={isLoading}
            error={error}
            searchText={searchText}
            onSearchTextChange={setSearchText}
            dateFilter={dateFilter}
            onDateFilterChange={setDateFilter}
            gitFilter={gitFilter}
            onGitFilterChange={setGitFilter}
            showDetailPanel={showDetailPanel}
            onToggleDetailPanel={handleToggleDetail}
            onOpenDashboard={() => setShowDashboard(true)}
            onOpenSettings={() => setShowSettings(true)}
            selectedProjects={selectedProjects}
            onSelectProject={handleSelectProject}
            onEnterWorkspace={handleEnterWorkspace}
            onTagSelected={handleSelectTag}
            onRemoveTagFromProject={removeTagFromProject}
            onRefreshProject={refreshProject}
            onCopyPath={handleCopyPath}
            onOpenInTerminal={handleOpenInTerminal}
            devTools={enabledDevTools}
            defaultDevToolId={appState.settings.defaultDevToolId}
            onOpenInDevTool={handleOpenInDevTool}
            onOpenInDefaultDevTool={handleOpenInDefaultDevTool}
            onMoveToRecycleBin={handleMoveProjectToRecycleBin}
            getTagColor={getTagColor}
            searchInputRef={searchInputRef}
          />
          {showDetailPanel ? (
            <DetailPanel
              project={resolvedSelectedProject}
              tags={appState.tags}
              onClose={() => setShowDetailPanel(false)}
              onAddTagToProject={addTagToProject}
              onRemoveTagFromProject={removeTagFromProject}
              getTagColor={getTagColor}
            />
          ) : null}
        </div>
      )}

      <TagEditDialog
        title={tagDialogState?.mode === "edit" ? "编辑标签" : "新建标签"}
        isOpen={Boolean(tagDialogState)}
        existingTags={appState.tags}
        initialName={tagDialogState?.tag?.name ?? ""}
        initialColor={tagDialogState?.tag ? getTagHex(tagDialogState.tag.color) : undefined}
        onClose={() => setTagDialogState(null)}
        onSubmit={(name, color) => void handleTagSubmit(name, color)}
      />

      {showRecycleBin ? (
        <RecycleBinModal
          items={recycleBinItems}
          onClose={() => setShowRecycleBin(false)}
          onRestore={handleRestoreProjectFromRecycleBin}
        />
      ) : null}

      {showDashboard ? (
        <DashboardModal
          projects={visibleProjects}
          tags={appState.tags}
          heatmapStore={heatmapStore}
          onClose={() => setShowDashboard(false)}
          onUpdateGitDaily={updateGitDaily}
        />
      ) : null}
      {showSettings ? (
        <SettingsModal
          settings={appState.settings}
          projects={visibleProjects}
          devToolPresets={devToolPresets}
          onClose={handleCloseSettings}
          onSaveSettings={handleSaveSettings}
          onPreviewTerminalRenderer={handlePreviewTerminalRenderer}
        />
      ) : null}
      {toast ? (
        <div className={`toast${toast.variant === "error" ? " is-error" : ""}`}>{toast.message}</div>
      ) : null}
    </div>
  );
}

/** 应用根组件，负责注入全局状态提供者。 */
function App() {
  return (
    <DevHavenProvider>
      <AppLayout />
    </DevHavenProvider>
  );
}

export default App;

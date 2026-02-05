import { useCallback, useEffect, useMemo, useState } from "react";

import type { AppStateFile, Project, TagData } from "../models/types";
import {
  buildProjects,
  discoverProjects,
  loadAppState,
  loadProjects,
  saveAppState,
  saveProjects,
} from "../services/appStorage";
import { collectGitDaily } from "../services/gitDaily";
import { pickColorForTag } from "../utils/tagColors";

const DEFAULT_WORKSPACE_ENABLED = typeof navigator !== "undefined" && /mac/i.test(navigator.userAgent);

const emptyState: AppStateFile = {
  version: 5,
  tags: [],
  directories: [],
  recycleBin: [],
  settings: {
    editorOpenTool: {
      commandPath: "",
      arguments: [],
    },
    terminalOpenTool: {
      commandPath: "",
      arguments: [],
    },
    terminalUseWebglRenderer: true,
    showMonitorWindow: false,
    workspaceEnabled: DEFAULT_WORKSPACE_ENABLED,
    gitIdentities: [],
    devTools: [],
    defaultDevToolId: "",
  },
};

const normalizeAppState = (state: AppStateFile): AppStateFile => ({
  ...state,
  recycleBin: state.recycleBin ?? [],
  settings: {
    ...state.settings,
    devTools: state.settings.devTools ?? [],
    defaultDevToolId: state.settings.defaultDevToolId ?? "",
    workspaceEnabled:
      typeof state.settings.workspaceEnabled === "boolean"
        ? state.settings.workspaceEnabled
        : DEFAULT_WORKSPACE_ENABLED,
  },
});

export type DevHavenState = {
  appState: AppStateFile;
  projects: Project[];
  isLoading: boolean;
  error: string | null;
};

export type DevHavenActions = {
  refresh: () => Promise<void>;
  addProjects: (paths: string[]) => Promise<void>;
  refreshProject: (path: string) => Promise<void>;
  updateGitDaily: (paths?: string[]) => Promise<void>;
  addDirectory: (path: string) => Promise<void>;
  removeDirectory: (path: string) => Promise<void>;
  moveProjectToRecycleBin: (path: string) => Promise<void>;
  restoreProjectFromRecycleBin: (path: string) => Promise<void>;
  updateSettings: (settings: AppStateFile["settings"]) => Promise<void>;
  updateTags: (tags: TagData[]) => Promise<void>;
  addTag: (name: string, colorHex?: string) => Promise<void>;
  renameTag: (from: string, to: string) => Promise<void>;
  removeTag: (name: string) => Promise<void>;
  toggleTagHidden: (name: string) => Promise<void>;
  setTagColor: (name: string, colorHex: string) => Promise<void>;
  addTagToProject: (projectId: string, tag: string) => Promise<void>;
  removeTagFromProject: (projectId: string, tag: string) => Promise<void>;
};

export type DevHavenStore = DevHavenState &
  DevHavenActions & {
    projectMap: Map<string, Project>;
  };

/** 项目管理主 Hook，封装状态、缓存与业务操作。 */
export function useDevHaven(): DevHavenStore {
  const [appState, setAppState] = useState<AppStateFile>(emptyState);
  const [projects, setProjects] = useState<Project[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const projectMap = useMemo(() => new Map(projects.map((project) => [project.id, project])), [projects]);

  /** 将项目中的标签同步到全局标签配置，并持久化。 */
  const syncTagsFromProjects = useCallback(
    async (state: AppStateFile, nextProjects: Project[]) => {
      const existing = new Map(state.tags.map((tag) => [tag.name, tag]));
      let changed = false;

      for (const project of nextProjects) {
        for (const tag of project.tags) {
          if (!existing.has(tag)) {
            existing.set(tag, {
              name: tag,
              color: hexToColorData(pickColorForTag(tag)),
              hidden: false,
            });
            changed = true;
          }
        }
      }

      if (!changed) {
        return state;
      }

      const nextState = {
        ...state,
        tags: Array.from(existing.values()),
      };

      setAppState(nextState);
      await saveAppState(nextState);
      return nextState;
    },
    [],
  );

  /** 刷新应用状态与项目列表，必要时触发扫描与构建。 */
  const refresh = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const [state, cachedProjects] = await Promise.all([loadAppState(), loadProjects()]);
      const resolvedState = normalizeAppState(state ?? emptyState);
      const resolvedProjects = cachedProjects ?? [];
      setAppState(resolvedState);
      if (resolvedState.directories.length === 0) {
        setProjects(resolvedProjects);
        await syncTagsFromProjects(resolvedState, resolvedProjects);
        return;
      }
      const paths = await discoverProjects(resolvedState.directories);
      const updatedProjects = await buildProjects(paths, resolvedProjects);
      setProjects(updatedProjects);
      await saveProjects(updatedProjects);
      await syncTagsFromProjects(resolvedState, updatedProjects);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, [syncTagsFromProjects]);

  /** 将指定路径直接合并进项目列表并更新标签。 */
  const addProjects = useCallback(
    async (paths: string[]) => {
      const uniquePaths = Array.from(new Set(paths.map((path) => path.trim()).filter(Boolean)));
      if (uniquePaths.length === 0) {
        return;
      }
      try {
        const updatedProjects = await buildProjects(uniquePaths, projects);
        const nextProjects = mergeProjectsByPath(projects, updatedProjects);
        setProjects(nextProjects);
        await saveProjects(nextProjects);
        await syncTagsFromProjects(appState, nextProjects);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [appState, projects, syncTagsFromProjects],
  );

  /** 重新扫描指定项目路径并更新缓存。 */
  const refreshProject = useCallback(
    async (path: string) => {
      if (!path) {
        return;
      }
      try {
        const updatedProjects = await buildProjects([path], projects);
        const nextProjects = mergeProjectsByPath(projects, updatedProjects);
        setProjects(nextProjects);
        await saveProjects(nextProjects);
        await syncTagsFromProjects(appState, nextProjects);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [appState, projects, syncTagsFromProjects],
  );

  /** 更新项目的 Git 每日提交统计（支持指定路径）。 */
  const updateGitDaily = useCallback(
    async (paths?: string[]) => {
      const hiddenPaths = new Set(appState.recycleBin);
      const targetPaths =
        paths && paths.length > 0
          ? paths.filter((path) => !hiddenPaths.has(path))
          : projects
              .filter((project) => project.git_commits > 0 && !hiddenPaths.has(project.path))
              .map((project) => project.path);
      if (targetPaths.length === 0) {
        return;
      }
      try {
        const results = await collectGitDaily(targetPaths, appState.settings.gitIdentities);
        if (results.length === 0) {
          return;
        }
        const byPath = new Map(results.map((result) => [result.path, result]));
        const nextProjects = projects.map((project) => {
          const match = byPath.get(project.path);
          if (!match || match.error) {
            return project;
          }
          return { ...project, git_daily: match.gitDaily ?? null };
        });
        setProjects(nextProjects);
        await saveProjects(nextProjects);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [appState.recycleBin, appState.settings.gitIdentities, projects],
  );

  /** 添加需要扫描的工作目录并持久化。 */
  const addDirectory = useCallback(
    async (path: string) => {
      const nextDirectories = Array.from(new Set([...appState.directories, path]));
      const nextState = { ...appState, directories: nextDirectories };
      setAppState(nextState);
      await saveAppState(nextState);
    },
    [appState],
  );

  /** 移除工作目录并持久化。 */
  const removeDirectory = useCallback(
    async (path: string) => {
      const nextDirectories = appState.directories.filter((item) => item !== path);
      const nextState = { ...appState, directories: nextDirectories };
      setAppState(nextState);
      await saveAppState(nextState);
    },
    [appState],
  );

  /** 将项目路径移入回收站并持久化。 */
  const moveProjectToRecycleBin = useCallback(
    async (path: string) => {
      if (!path) {
        return;
      }
      const nextRecycleBin = Array.from(new Set([...appState.recycleBin, path]));
      const nextState = { ...appState, recycleBin: nextRecycleBin };
      setAppState(nextState);
      await saveAppState(nextState);
    },
    [appState],
  );

  /** 从回收站恢复项目路径并持久化。 */
  const restoreProjectFromRecycleBin = useCallback(
    async (path: string) => {
      if (!path) {
        return;
      }
      const nextRecycleBin = appState.recycleBin.filter((item) => item !== path);
      const nextState = { ...appState, recycleBin: nextRecycleBin };
      setAppState(nextState);
      await saveAppState(nextState);
    },
    [appState],
  );

  /** 批量更新标签配置并持久化。 */
  const updateTags = useCallback(
    async (tags: TagData[]) => {
      const nextState = { ...appState, tags };
      setAppState(nextState);
      await saveAppState(nextState);
    },
    [appState],
  );

  /** 更新应用设置并持久化。 */
  const updateSettings = useCallback(
    async (settings: AppStateFile["settings"]) => {
      const nextState = { ...appState, settings };
      setAppState(nextState);
      await saveAppState(nextState);
    },
    [appState],
  );

  /** 新建标签并自动分配颜色。 */
  const addTag = useCallback(
    async (name: string, colorHex?: string) => {
      const normalized = name.trim();
      if (!normalized) {
        return;
      }
      if (appState.tags.some((tag) => tag.name === normalized)) {
        return;
      }
      const nextState = {
        ...appState,
        tags: [
          ...appState.tags,
          {
            name: normalized,
            color: hexToColorData(colorHex ?? pickColorForTag(normalized)),
            hidden: false,
          },
        ],
      };
      setAppState(nextState);
      await saveAppState(nextState);
    },
    [appState],
  );

  /** 重命名标签，同时更新项目上的标签引用。 */
  const renameTag = useCallback(
    async (from: string, to: string) => {
      const normalized = to.trim();
      if (!normalized || from === normalized) {
        return;
      }
      if (appState.tags.some((tag) => tag.name === normalized)) {
        return;
      }

      const nextTags = appState.tags.map((tag) =>
        tag.name === from ? { ...tag, name: normalized } : tag,
      );
      const nextProjects = projects.map((project) =>
        project.tags.includes(from)
          ? { ...project, tags: project.tags.map((tag) => (tag === from ? normalized : tag)) }
          : project,
      );

      setAppState({ ...appState, tags: nextTags });
      setProjects(nextProjects);
      await Promise.all([saveAppState({ ...appState, tags: nextTags }), saveProjects(nextProjects)]);
    },
    [appState, projects],
  );

  /** 删除标签并同步移除项目引用。 */
  const removeTag = useCallback(
    async (name: string) => {
      const nextTags = appState.tags.filter((tag) => tag.name !== name);
      const nextProjects = projects.map((project) => ({
        ...project,
        tags: project.tags.filter((tag) => tag !== name),
      }));
      setAppState({ ...appState, tags: nextTags });
      setProjects(nextProjects);
      await Promise.all([saveAppState({ ...appState, tags: nextTags }), saveProjects(nextProjects)]);
    },
    [appState, projects],
  );

  /** 切换标签的隐藏状态。 */
  const toggleTagHidden = useCallback(
    async (name: string) => {
      const nextTags = appState.tags.map((tag) =>
        tag.name === name ? { ...tag, hidden: !tag.hidden } : tag,
      );
      setAppState({ ...appState, tags: nextTags });
      await saveAppState({ ...appState, tags: nextTags });
    },
    [appState],
  );

  /** 更新标签颜色配置。 */
  const setTagColor = useCallback(
    async (name: string, colorHex: string) => {
      const nextTags = appState.tags.map((tag) =>
        tag.name === name ? { ...tag, color: hexToColorData(colorHex) } : tag,
      );
      setAppState({ ...appState, tags: nextTags });
      await saveAppState({ ...appState, tags: nextTags });
    },
    [appState],
  );

  /** 为指定项目添加标签并同步全局标签。 */
  const addTagToProject = useCallback(
    async (projectId: string, tag: string) => {
      const nextProjects = projects.map((project) =>
        project.id === projectId && !project.tags.includes(tag)
          ? { ...project, tags: [...project.tags, tag] }
          : project,
      );
      setProjects(nextProjects);
      await saveProjects(nextProjects);
      await syncTagsFromProjects(appState, nextProjects);
    },
    [appState, projects, syncTagsFromProjects],
  );

  /** 从指定项目移除标签。 */
  const removeTagFromProject = useCallback(
    async (projectId: string, tag: string) => {
      const nextProjects = projects.map((project) =>
        project.id === projectId ? { ...project, tags: project.tags.filter((item) => item !== tag) } : project,
      );
      setProjects(nextProjects);
      await saveProjects(nextProjects);
    },
    [projects],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    appState,
    projects,
    isLoading,
    error,
    projectMap,
    refresh,
    addProjects,
    refreshProject,
    updateGitDaily,
    addDirectory,
    removeDirectory,
    moveProjectToRecycleBin,
    restoreProjectFromRecycleBin,
    updateSettings,
    updateTags,
    addTag,
    renameTag,
    removeTag,
    toggleTagHidden,
    setTagColor,
    addTagToProject,
    removeTagFromProject,
  };
}

/** 将 Hex 颜色转换为可存储的 RGBA 结构。 */
function hexToColorData(hex: string) {
  const value = hex.replace("#", "");
  if (value.length !== 6) {
    return { r: 0.3, g: 0.3, b: 0.3, a: 1 };
  }
  const r = parseInt(value.slice(0, 2), 16) / 255;
  const g = parseInt(value.slice(2, 4), 16) / 255;
  const b = parseInt(value.slice(4, 6), 16) / 255;
  return { r, g, b, a: 1 };
}

/** 按路径合并新旧项目，保留未更新的旧项目。 */
function mergeProjectsByPath(existing: Project[], updates: Project[]) {
  const updatesByPath = new Map(updates.map((project) => [project.path, project]));
  const nextProjects = existing.map((project) => updatesByPath.get(project.path) ?? project);
  for (const project of updates) {
    if (!existing.some((item) => item.path === project.path)) {
      nextProjects.push(project);
    }
  }
  return nextProjects;
}

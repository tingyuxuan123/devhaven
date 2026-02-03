import type { DevTool, Project } from "../models/types";
import type { DateFilter, GitFilter } from "../models/filters";
import { DATE_FILTER_OPTIONS, GIT_FILTER_OPTIONS } from "../models/filters";
import ProjectCard from "./ProjectCard";
import SearchBar from "./SearchBar";
import DropdownMenu from "./DropdownMenu";
import {
  IconCalendar,
  IconChartLine,
  IconSearch,
  IconSettings,
  IconSidebarRight,
} from "./Icons";

export type MainContentProps = {
  projects: Project[];
  filteredProjects: Project[];
  recycleBinCount: number;
  isLoading: boolean;
  error: string | null;
  searchText: string;
  onSearchTextChange: (value: string) => void;
  dateFilter: DateFilter;
  onDateFilterChange: (value: DateFilter) => void;
  gitFilter: GitFilter;
  onGitFilterChange: (value: GitFilter) => void;
  showDetailPanel: boolean;
  onToggleDetailPanel: () => void;
  onOpenDashboard: () => void;
  onOpenSettings: () => void;
  selectedProjects: Set<string>;
  onSelectProject: (project: Project, event: React.MouseEvent<HTMLDivElement>) => void;
  onEnterWorkspace: (project: Project) => void;
  onTagSelected: (tag: string) => void;
  onRemoveTagFromProject: (projectId: string, tag: string) => void;
  onRefreshProject: (path: string) => void;
  onCopyPath: (path: string) => void;
  onOpenInTerminal: (path: string) => void;
  devTools: DevTool[];
  defaultDevToolId: string;
  onOpenInDevTool: (tool: DevTool, path: string) => void;
  onOpenInDefaultDevTool: (path: string) => void;
  onMoveToRecycleBin: (project: Project) => void;
  getTagColor: (tag: string) => string;
  searchInputRef: React.RefObject<HTMLInputElement | null>;
};

/** 主内容区，负责搜索过滤与项目列表展示。 */
export default function MainContent({
  projects,
  filteredProjects,
  recycleBinCount,
  isLoading,
  error,
  searchText,
  onSearchTextChange,
  dateFilter,
  onDateFilterChange,
  gitFilter,
  onGitFilterChange,
  showDetailPanel,
  onToggleDetailPanel,
  onOpenDashboard,
  onOpenSettings,
  selectedProjects,
  onSelectProject,
  onEnterWorkspace,
  onTagSelected,
  onRemoveTagFromProject,
  onRefreshProject,
  onCopyPath,
  onOpenInTerminal,
  devTools,
  defaultDevToolId,
  onOpenInDevTool,
  onOpenInDefaultDevTool,
  onMoveToRecycleBin,
  getTagColor,
  searchInputRef,
}: MainContentProps) {
  const currentDateOption = DATE_FILTER_OPTIONS.find((option) => option.value === dateFilter) ?? DATE_FILTER_OPTIONS[0];
  return (
    <section className="main-panel">
      <div className="search-toolbar">
        <button className="icon-button" aria-label="仪表盘" onClick={onOpenDashboard}>
          <IconChartLine size={18} />
        </button>
        <button
          className={`icon-button${showDetailPanel ? " is-active" : ""}`}
          aria-label="详情面板"
          onClick={onToggleDetailPanel}
        >
          <IconSidebarRight size={18} />
        </button>
        <button className="icon-button" aria-label="设置" onClick={onOpenSettings}>
          <IconSettings size={18} />
        </button>
        <SearchBar value={searchText} onChange={onSearchTextChange} ref={searchInputRef} />
        <DropdownMenu
          label={
            <span className="filter-select-label">
              <IconCalendar className="filter-icon" size={14} />
              <span>{currentDateOption.title}</span>
            </span>
          }
          items={DATE_FILTER_OPTIONS.map((option) => ({
            label: option.title,
            active: option.value === dateFilter,
            onClick: () => onDateFilterChange(option.value),
          }))}
          align="left"
          ariaLabel="日期筛选"
          triggerClassName="filter-select-trigger"
        />
        <div className="git-filter-group">
          {GIT_FILTER_OPTIONS.map((option) => (
            <button
              key={option.value}
              className={`git-filter-button${gitFilter === option.value ? " is-active" : ""}`}
              onClick={() => onGitFilterChange(option.value)}
            >
              {option.title}
            </button>
          ))}
        </div>
      </div>

      <div className="main-scroll">
        {isLoading ? (
          <div className="empty-state">正在加载项目数据...</div>
        ) : error ? (
          <div className="empty-state">{error}</div>
        ) : projects.length === 0 ? (
          <div className="empty-state">
            {recycleBinCount > 0 ? (
              <>
                <div>当前没有可见项目</div>
                <div>可在左侧回收站恢复隐藏项目</div>
              </>
            ) : (
              <>
                <div>暂未添加项目目录</div>
                <div>请在左侧添加工作目录或直接导入项目</div>
              </>
            )}
          </div>
        ) : filteredProjects.length === 0 ? (
          <div className="empty-state">
            <IconSearch className="empty-state-icon" size={36} />
            <div>没有匹配的项目</div>
            <div>尝试修改搜索条件或清除标签筛选</div>
          </div>
        ) : (
          <div className="project-grid">
            {filteredProjects.map((project) => (
              <ProjectCard
                key={project.id}
                project={project}
                isSelected={selectedProjects.has(project.id)}
                selectedProjectIds={selectedProjects}
                onSelect={(event) => onSelectProject(project, event)}
                onEnterWorkspace={() => onEnterWorkspace(project)}
                onTagClick={onTagSelected}
                onRemoveTag={onRemoveTagFromProject}
                getTagColor={getTagColor}
                onRefreshProject={onRefreshProject}
                onCopyPath={onCopyPath}
                onOpenInTerminal={onOpenInTerminal}
                devTools={devTools}
                defaultDevToolId={defaultDevToolId}
                onOpenInDevTool={onOpenInDevTool}
                onOpenInDefaultDevTool={onOpenInDefaultDevTool}
                onMoveToRecycleBin={onMoveToRecycleBin}
              />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

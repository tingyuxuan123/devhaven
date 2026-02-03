export type SwiftDate = number;

export type ColorData = {
  r: number;
  g: number;
  b: number;
  a: number;
};

export type TagData = {
  name: string;
  color: ColorData;
  hidden: boolean;
};

export type OpenToolSettings = {
  commandPath: string;
  arguments: string[];
};

export type DevTool = {
  id: string;
  name: string;
  commandPath: string;
  arguments: string[];
  enabled: boolean;
  isPreset: boolean;
};

export type DevToolPreset = {
  id: string;
  name: string;
  commandPath: string;
  arguments: string[];
};

export type GitIdentity = {
  name: string;
  email: string;
};

export type AppSettings = {
  editorOpenTool: OpenToolSettings;
  terminalOpenTool: OpenToolSettings;
  terminalUseWebglRenderer: boolean;
  showMonitorWindow: boolean;
  gitIdentities: GitIdentity[];
  devTools: DevTool[];
  defaultDevToolId: string;
};

export type AppStateFile = {
  version: number;
  tags: TagData[];
  directories: string[];
  recycleBin: string[];
  settings: AppSettings;
};

export type Project = {
  id: string;
  name: string;
  path: string;
  tags: string[];
  mtime: SwiftDate;
  size: number;
  checksum: string;
  git_commits: number;
  git_last_commit: SwiftDate;
  git_daily?: string | null;
  created: SwiftDate;
  checked: SwiftDate;
};

const APPLE_REFERENCE_EPOCH_MS = Date.UTC(2001, 0, 1, 0, 0, 0, 0);

/** 将 Swift 时间戳（以 2001-01-01 为起点）转为 JS Date。 */
export function swiftDateToJsDate(swiftDate: SwiftDate): Date {
  return new Date(APPLE_REFERENCE_EPOCH_MS + swiftDate * 1000);
}

/** 将 JS Date 转为 Swift 时间戳（以 2001-01-01 为起点）。 */
export function jsDateToSwiftDate(date: Date): SwiftDate {
  return (date.getTime() - APPLE_REFERENCE_EPOCH_MS) / 1000;
}

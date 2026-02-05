import { getVersion } from "@tauri-apps/api/app";

const RELEASES_URL = "https://api.github.com/repos/tingyuxuan123/devhaven/releases/latest";

type ReleaseAsset = {
  browser_download_url?: string;
  name?: string;
  content_type?: string;
};

type ReleaseResponse = {
  tag_name?: string;
  name?: string;
  html_url?: string;
  assets?: ReleaseAsset[];
};

export type UpdateCheckResult =
  | { status: "latest"; currentVersion: string; latestVersion: string; url?: string; downloadUrl?: string }
  | { status: "update"; currentVersion: string; latestVersion: string; url?: string; downloadUrl?: string }
  | { status: "error"; currentVersion: string; message: string };

/** 检查是否有新版本发布。 */
export async function checkForUpdates(): Promise<UpdateCheckResult> {
  const currentVersion = await getVersion();
  try {
    const response = await fetch(RELEASES_URL, {
      headers: {
        Accept: "application/vnd.github+json",
      },
    });
    if (!response.ok) {
      return {
        status: "error",
        currentVersion,
        message: `更新服务返回异常（${response.status}）`,
      };
    }
    const payload = (await response.json()) as ReleaseResponse;
    const latestRaw = payload.tag_name ?? payload.name ?? "";
    const latestVersion = normalizeVersion(latestRaw);
    if (!latestVersion) {
      return {
        status: "error",
        currentVersion,
        message: "未获取到版本信息",
      };
    }
    const normalizedCurrent = normalizeVersion(currentVersion);
    const isUpdate = compareVersions(latestVersion, normalizedCurrent) > 0;
    const downloadAsset = selectDownloadAsset(payload.assets ?? []);
    const downloadUrl = downloadAsset?.browser_download_url;
    return {
      status: isUpdate ? "update" : "latest",
      currentVersion,
      latestVersion,
      url: payload.html_url,
      downloadUrl,
    };
  } catch (error) {
    return {
      status: "error",
      currentVersion,
      message: error instanceof Error ? error.message : String(error),
    };
  }
}

function normalizeVersion(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }
  const sanitized = trimmed.startsWith("v") ? trimmed.slice(1) : trimmed;
  const match = sanitized.match(/\d+(?:\.\d+){0,2}/);
  return match ? match[0] : sanitized;
}

function compareVersions(left: string, right: string): number {
  const leftParts = parseVersionParts(left);
  const rightParts = parseVersionParts(right);
  const maxLength = Math.max(leftParts.length, rightParts.length);
  for (let index = 0; index < maxLength; index += 1) {
    const leftValue = leftParts[index] ?? 0;
    const rightValue = rightParts[index] ?? 0;
    if (leftValue > rightValue) {
      return 1;
    }
    if (leftValue < rightValue) {
      return -1;
    }
  }
  return 0;
}

function selectDownloadAsset(assets: ReleaseAsset[]): ReleaseAsset | undefined {
  if (assets.length === 0) {
    return undefined;
  }
  const prioritized = assets.find((asset) => asset.browser_download_url?.toLowerCase().endsWith(".msi"));
  if (prioritized) {
    return prioritized;
  }
  const executable = assets.find((asset) => asset.browser_download_url?.toLowerCase().endsWith(".exe"));
  if (executable) {
    return executable;
  }
  return assets[0];
}

function parseVersionParts(value: string): number[] {
  if (!value) {
    return [];
  }
  return value
    .split(".")
    .map((part) => Number.parseInt(part, 10))
    .filter((part) => Number.isFinite(part));
}

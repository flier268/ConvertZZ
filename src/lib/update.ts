export interface ReleaseUpdate {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  url: string;
}

export interface InstallableUpdate {
  currentVersion: string;
  version: string;
  body?: string;
}

export type ResolvedUpdate =
  | { kind: "none"; currentVersion: string; latestVersion: string }
  | { kind: "install"; currentVersion: string; latestVersion: string; notes?: string }
  | { kind: "open"; currentVersion: string; latestVersion: string; url: string };

const LATEST_RELEASE_API = "https://api.github.com/repos/flier268/ConvertZZ/releases/latest";
export const FALLBACK_RELEASE_URL = "https://github.com/flier268/ConvertZZ/releases";

export async function checkLatestRelease(
  currentVersion: string,
  fetcher: typeof fetch = fetch,
): Promise<ReleaseUpdate> {
  const response = await fetcher(LATEST_RELEASE_API, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!response.ok) throw new Error(`GitHub Release 檢查失敗：HTTP ${response.status}`);
  const payload = (await response.json()) as { tag_name?: unknown; html_url?: unknown };
  const latestVersion = normalizeVersion(
    typeof payload.tag_name === "string" ? payload.tag_name : "",
  );
  if (!latestVersion) throw new Error("GitHub Release 未提供有效版本號。");
  const url =
    typeof payload.html_url === "string" && payload.html_url.startsWith("https://github.com/")
      ? payload.html_url
      : FALLBACK_RELEASE_URL;
  return {
    currentVersion: normalizeVersion(currentVersion),
    latestVersion,
    updateAvailable: compareVersions(latestVersion, normalizeVersion(currentVersion)) > 0,
    url,
  };
}

export async function resolveUpdate(
  currentVersion: string,
  options: {
    checkInstallable?: () => Promise<InstallableUpdate | null>;
    checkRelease?: typeof checkLatestRelease;
    fetcher?: typeof fetch;
  } = {},
): Promise<ResolvedUpdate> {
  const checkRelease = options.checkRelease ?? checkLatestRelease;
  if (options.checkInstallable) {
    try {
      const installable = await options.checkInstallable();
      if (installable) {
        return {
          kind: "install",
          currentVersion: normalizeVersion(installable.currentVersion || currentVersion),
          latestVersion: normalizeVersion(installable.version),
          notes: installable.body,
        };
      }
    } catch {
      // 簽署更新通道不可用時改走 GitHub Release。
    }
  }
  const release = await checkRelease(currentVersion, options.fetcher);
  if (!release.updateAvailable) {
    return {
      kind: "none",
      currentVersion: release.currentVersion,
      latestVersion: release.latestVersion,
    };
  }
  return {
    kind: "open",
    currentVersion: release.currentVersion,
    latestVersion: release.latestVersion,
    url: release.url,
  };
}

export function compareVersions(left: string, right: string): number {
  const a = versionParts(left);
  const b = versionParts(right);
  for (let index = 0; index < Math.max(a.length, b.length); index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference) return Math.sign(difference);
  }
  return 0;
}

function normalizeVersion(value: string): string {
  return value.trim().replace(/^v/iu, "").split("-")[0];
}

function versionParts(value: string): number[] {
  return normalizeVersion(value)
    .split(".")
    .map((part) => Number.parseInt(part, 10) || 0);
}

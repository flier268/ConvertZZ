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

export interface VersionCheckOptions {
  includePreRelease?: boolean;
}

export const PRE_RELEASE_CHANNEL_TAGS = ["alpha", "beta", "rc"] as const;
export type PreReleaseChannel = (typeof PRE_RELEASE_CHANNEL_TAGS)[number];

type PreReleasePart = string | number;

interface ParsedVersion {
  core: [number, number, number];
  pre: PreReleasePart[] | null;
}

const GITHUB_REPO = "flier268/ConvertZZ";
const GITHUB_API = `https://api.github.com/repos/${GITHUB_REPO}`;
const LATEST_RELEASE_API = `${GITHUB_API}/releases/latest`;
const GITHUB_HEADERS = { headers: { Accept: "application/vnd.github+json" } };
export const FALLBACK_RELEASE_URL = `https://github.com/${GITHUB_REPO}/releases`;

const VERSION_RE =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?$/u;

export function normalizeVersion(value: string): string {
  const trimmed = value.trim().replace(/^v/iu, "");
  const plus = trimmed.indexOf("+");
  return plus === -1 ? trimmed : trimmed.slice(0, plus);
}

export function isPreReleaseVersion(value: string): boolean {
  const parsed = parseVersion(value);
  return Boolean(parsed?.pre);
}

export function channelFromReleaseTag(tag: string): PreReleaseChannel | null {
  const normalized = normalizeVersion(tag);
  if (!normalized || (PRE_RELEASE_CHANNEL_TAGS as readonly string[]).includes(normalized)) {
    return null;
  }
  const parsed = parseVersion(normalized);
  const pre = parsed?.pre
    ? parsed.pre.map(String).join(".").toLowerCase()
    : (() => {
        const dash = normalized.indexOf("-");
        return dash === -1 ? "" : normalized.slice(dash + 1).toLowerCase();
      })();
  if (!pre) return null;
  let best: PreReleaseChannel | null = null;
  let bestPos = Infinity;
  for (const channel of PRE_RELEASE_CHANNEL_TAGS) {
    const pos = pre.indexOf(channel);
    if (pos >= 0 && pos < bestPos) {
      best = channel;
      bestPos = pos;
    }
  }
  return best;
}

export function signedUpdateManifestUrl(version: string): string {
  return `https://github.com/${GITHUB_REPO}/releases/download/v${normalizeVersion(version)}/latest.json`;
}

export function parseVersion(value: string): ParsedVersion | null {
  const normalized = normalizeVersion(value);
  const match = VERSION_RE.exec(normalized);
  if (!match) return null;
  return {
    core: [Number(match[1]), Number(match[2]), Number(match[3])],
    pre: match[4] ? match[4].split(".").map(parsePreReleasePart) : null,
  };
}

export function compareVersions(left: string, right: string): number {
  const a = parseVersion(left);
  const b = parseVersion(right);
  if (!a && !b) return 0;
  if (!a) return -1;
  if (!b) return 1;
  for (let index = 0; index < 3; index += 1) {
    const difference = a.core[index]! - b.core[index]!;
    if (difference) return Math.sign(difference);
  }
  if (a.pre === null && b.pre === null) return 0;
  if (a.pre === null) return 1;
  if (b.pre === null) return -1;
  const length = Math.max(a.pre.length, b.pre.length);
  for (let index = 0; index < length; index += 1) {
    const leftPart = a.pre[index];
    const rightPart = b.pre[index];
    if (leftPart === undefined) return -1;
    if (rightPart === undefined) return 1;
    if (leftPart === rightPart) continue;
    if (typeof leftPart === "number" && typeof rightPart === "number") {
      return Math.sign(leftPart - rightPart);
    }
    if (typeof leftPart === "number") return -1;
    if (typeof rightPart === "number") return 1;
    return leftPart < rightPart ? -1 : 1;
  }
  return 0;
}

export async function checkLatestRelease(
  currentVersion: string,
  fetcher: typeof fetch = fetch,
  options: VersionCheckOptions = {},
): Promise<ReleaseUpdate> {
  const includePreRelease = options.includePreRelease === true;
  const current = normalizeVersion(currentVersion);
  const release = includePreRelease
    ? await fetchNewestRelease(fetcher, true)
    : await fetchNewestRelease(fetcher, false);
  return {
    currentVersion: current,
    latestVersion: release.version,
    updateAvailable: compareVersions(release.version, current) > 0,
    url: release.url,
  };
}

export async function resolveUpdate(
  currentVersion: string,
  options: {
    includePreRelease?: boolean;
    checkInstallable?: (targetVersion?: string) => Promise<InstallableUpdate | null>;
    checkRelease?: typeof checkLatestRelease;
    fetcher?: typeof fetch;
  } = {},
): Promise<ResolvedUpdate> {
  const includePreRelease = options.includePreRelease === true;
  const checkRelease = options.checkRelease ?? checkLatestRelease;
  const current = normalizeVersion(currentVersion);

  let release: ReleaseUpdate | undefined;
  let releaseError: unknown;
  try {
    release = await checkRelease(currentVersion, options.fetcher, { includePreRelease });
  } catch (error) {
    releaseError = error;
  }

  if (options.checkInstallable) {
    try {
      const installable = await options.checkInstallable(release?.latestVersion);
      if (installable) {
        const installVersion = normalizeVersion(installable.version);
        const installCurrent = normalizeVersion(installable.currentVersion || current);
        const newer = compareVersions(installVersion, installCurrent) > 0;
        const allowedChannel = includePreRelease || !isPreReleaseVersion(installVersion);
        const matchesReleaseTarget =
          !release?.updateAvailable || compareVersions(installVersion, release.latestVersion) === 0;
        if (newer && allowedChannel && (matchesReleaseTarget || !release)) {
          return {
            kind: "install",
            currentVersion: installCurrent,
            latestVersion: installVersion,
            notes: installable.body,
          };
        }
      }
    } catch {
      // 簽署更新通道不可用時改走 GitHub Release。
    }
  }

  if (!release) {
    throw releaseError instanceof Error
      ? releaseError
      : new Error(releaseError ? String(releaseError) : "GitHub Release 檢查失敗。");
  }

  const resolvedCurrent = normalizeVersion(release.currentVersion || current);
  if (!release.updateAvailable) {
    return {
      kind: "none",
      currentVersion: resolvedCurrent,
      latestVersion: release.latestVersion || resolvedCurrent,
    };
  }

  return {
    kind: "open",
    currentVersion: resolvedCurrent,
    latestVersion: release.latestVersion,
    url: release.url,
  };
}

export function isUpdateVersionSkipped(
  latestVersion: string,
  skippedVersion: string | undefined,
): boolean {
  const skipped = skippedVersion?.trim();
  if (!skipped) return false;
  return compareVersions(latestVersion, skipped) <= 0;
}

function parsePreReleasePart(part: string): PreReleasePart {
  if (/^(0|[1-9]\d*)$/u.test(part)) return Number(part);
  return part;
}

async function fetchNewestRelease(
  fetcher: typeof fetch,
  includePreRelease: boolean,
): Promise<{ version: string; url: string }> {
  if (!includePreRelease) {
    const official = await fetchOfficialRelease(fetcher, { allowMissing: false });
    if (!official) throw new Error("GitHub Release 未提供有效正式版本。");
    return official;
  }

  const [official, ...channels] = await Promise.all([
    fetchOfficialRelease(fetcher, { allowMissing: true }),
    ...PRE_RELEASE_CHANNEL_TAGS.map((channel) => fetchChannelRelease(fetcher, channel)),
  ]);

  let best: { version: string; url: string } | undefined;
  for (const candidate of [official, ...channels]) {
    if (!candidate) continue;
    if (!best || compareVersions(candidate.version, best.version) > 0) {
      best = candidate;
    }
  }
  if (!best) throw new Error("GitHub Release 未提供有效版本號。");
  return best;
}

async function fetchOfficialRelease(
  fetcher: typeof fetch,
  options: { allowMissing: boolean },
): Promise<{ version: string; url: string } | null> {
  const result = await githubJson(fetcher, LATEST_RELEASE_API);
  if (!result.ok) {
    if (options.allowMissing && result.status === 404) return null;
    throw new Error(`GitHub Release 檢查失敗：HTTP ${result.status}`);
  }
  const payload = result.data as {
    tag_name?: unknown;
    html_url?: unknown;
    prerelease?: unknown;
    draft?: unknown;
  };
  if (payload.draft === true || payload.prerelease === true) {
    if (options.allowMissing) return null;
    throw new Error("GitHub Release 未提供有效正式版本。");
  }
  const version = normalizeVersion(typeof payload.tag_name === "string" ? payload.tag_name : "");
  if (!version || !parseVersion(version) || isPreReleaseVersion(version)) {
    if (options.allowMissing) return null;
    throw new Error("GitHub Release 未提供有效版本號。");
  }
  return { version, url: releasePageUrl(payload.html_url) };
}

async function fetchChannelRelease(
  fetcher: typeof fetch,
  channel: PreReleaseChannel,
): Promise<{ version: string; url: string } | null> {
  const refResult = await githubJson(fetcher, `${GITHUB_API}/git/ref/tags/${channel}`);
  if (!refResult.ok) return null;
  const ref = refResult.data as { object?: { type?: unknown; sha?: unknown } };
  const sha = typeof ref.object?.sha === "string" ? ref.object.sha : "";
  if (!sha || ref.object?.type !== "tag") return null;

  const tagResult = await githubJson(fetcher, `${GITHUB_API}/git/tags/${sha}`);
  if (!tagResult.ok) return null;
  const message = (tagResult.data as { message?: unknown }).message;
  const versionTag =
    typeof message === "string" ? (message.trim().split(/\r?\n/u)[0] ?? "").trim() : "";
  const version = normalizeVersion(versionTag);
  if (!version || !parseVersion(version) || channelFromReleaseTag(version) !== channel) {
    return null;
  }
  return { version, url: versionedReleaseUrl(versionTag) };
}

async function githubJson(
  fetcher: typeof fetch,
  url: string,
): Promise<{ ok: true; data: unknown } | { ok: false; status: number }> {
  const response = await fetcher(url, GITHUB_HEADERS);
  if (response.status === 404) return { ok: false, status: 404 };
  if (!response.ok) throw new Error(`GitHub Release 檢查失敗：HTTP ${response.status}`);
  return { ok: true, data: await response.json() };
}

function versionedReleaseUrl(versionTag: string): string {
  const trimmed = versionTag.trim();
  const tag = /^v/iu.test(trimmed) ? trimmed : `v${normalizeVersion(trimmed)}`;
  return `https://github.com/${GITHUB_REPO}/releases/tag/${tag}`;
}

function releasePageUrl(value: unknown): string {
  return typeof value === "string" && value.startsWith("https://github.com/")
    ? value
    : FALLBACK_RELEASE_URL;
}

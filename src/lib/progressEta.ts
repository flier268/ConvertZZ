import { t } from "@/i18n";

export type ProgressSnapshot = {
  current: number;
  total: number;
  message: string;
};

/** 依已過時間與進度估算剩餘秒數；不足樣本時回傳 null。 */
export function estimateRemainingSeconds(
  progress: ProgressSnapshot | undefined,
  startedAtMs: number | undefined,
  nowMs = Date.now(),
): number | null {
  if (!progress || startedAtMs == null) return null;
  const total = Math.max(1, progress.total);
  const current = Math.max(0, Math.min(progress.current, total));
  if (current <= 0) return null;
  const elapsedSec = (nowMs - startedAtMs) / 1000;
  if (elapsedSec < 0.4) return null;
  const rate = current / elapsedSec;
  if (rate <= 0) return null;
  const remaining = (total - current) / rate;
  if (!Number.isFinite(remaining) || remaining < 0) return null;
  return remaining;
}

export function formatDurationSeconds(totalSeconds: number): string {
  const seconds = Math.max(0, Math.round(totalSeconds));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  if (hours > 0) return t("lib.progress.hms", { hours, minutes, seconds: rest });
  if (minutes > 0) return t("lib.progress.ms", { minutes, seconds: rest });
  return t("lib.progress.s", { seconds: rest });
}

export function formatProgressLabel(
  progress: ProgressSnapshot | undefined,
  startedAtMs: number | undefined,
  nowMs = Date.now(),
): string {
  const base = progress?.message?.trim() || t("lib.progress.processing");
  const remaining = estimateRemainingSeconds(progress, startedAtMs, nowMs);
  if (remaining == null) return base;
  if (remaining < 1) return t("lib.progress.almostDone", { base });
  return t("lib.progress.remaining", { base, duration: formatDurationSeconds(remaining) });
}

export function progressPercentage(progress: ProgressSnapshot | undefined): number {
  if (!progress) return 0;
  return Math.round((progress.current / Math.max(1, progress.total)) * 100);
}

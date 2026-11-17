import { t } from "@/i18n";

function safeJson(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return t("lib.errors.unserializable");
  }
}

function nonEmptyString(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed ? trimmed : undefined;
}

/**
 * 把 invoke／plugin／未知拋出值收成可讀字串。
 * Tauri 常直接 reject 字串或 `{ message, code }`，不一定是 Error。
 */
export function formatUnknownError(error: unknown): string {
  if (error == null) return t("lib.errors.nullValue");
  if (typeof error === "string") return nonEmptyString(error) ?? t("lib.errors.emptyString");
  if (typeof error === "number" || typeof error === "boolean" || typeof error === "bigint") {
    return String(error);
  }

  if (error instanceof Error) {
    const extras = error as Error & { code?: unknown; details?: unknown };
    const parts: string[] = [];
    const message = nonEmptyString(error.message);
    if (message) parts.push(message);
    else parts.push(t("lib.errors.emptyMessage"));
    if (error.name && error.name !== "Error") parts.push(`name=${error.name}`);
    if (extras.code != null && extras.code !== "") parts.push(`code=${String(extras.code)}`);
    if (extras.details != null) parts.push(`details=${safeJson(extras.details)}`);
    return parts.join(" · ");
  }

  if (typeof error === "object") {
    const record = error as { message?: unknown; code?: unknown; details?: unknown };
    const message = nonEmptyString(record.message);
    const parts: string[] = [];
    if (message) parts.push(message);
    if (record.code != null && record.code !== "") parts.push(`code=${String(record.code)}`);
    if (record.details != null) parts.push(`details=${safeJson(record.details)}`);
    if (parts.length) return parts.join(" · ");
    const json = safeJson(error);
    if (json && json !== "{}" && json !== "[]") return json;
    return t("lib.errors.emptyObject");
  }

  return String(error);
}

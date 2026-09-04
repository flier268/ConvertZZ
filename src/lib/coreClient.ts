import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CoreOperation } from "@shared/contracts";
import { formatUnknownError } from "./errors";

export type Progress = { current: number; total: number; message: string };

export type CoreRequestOptions = {
  /** 0 或省略表示不設前端逾時。 */
  timeoutMs?: number;
  onProgress?: (progress: Progress) => void;
  onRequestId?: (requestId: string) => void;
};

type PendingRequest = {
  onProgress?: (progress: Progress) => void;
};

function resolveOptions(
  timeoutOrOptions?: number | CoreRequestOptions,
  onProgress?: (progress: Progress) => void,
): CoreRequestOptions {
  if (typeof timeoutOrOptions === "number") {
    return { timeoutMs: timeoutOrOptions, onProgress };
  }
  if (timeoutOrOptions) {
    return {
      timeoutMs: timeoutOrOptions.timeoutMs,
      onProgress: timeoutOrOptions.onProgress ?? onProgress,
      onRequestId: timeoutOrOptions.onRequestId,
    };
  }
  return { onProgress };
}

class CoreClient {
  private readonly pending = new Map<string, PendingRequest>();
  private initialized?: Promise<void>;

  request<T>(
    operation: CoreOperation,
    payload: unknown,
    timeoutOrOptions?: number | CoreRequestOptions,
    onProgress?: (progress: Progress) => void,
  ): Promise<T> {
    const options = resolveOptions(timeoutOrOptions, onProgress);
    const timeoutMs = options.timeoutMs ?? 0;
    return new Promise<T>(async (resolve, reject) => {
      let id: string | undefined;
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        await this.initialize();
        const requestId = crypto.randomUUID();
        id = requestId;
        options.onRequestId?.(requestId);
        if (timeoutMs > 0) {
          timer = setTimeout(() => {
            this.pending.delete(requestId);
            reject(new Error(`轉換核心要求逾時：${operation}`));
          }, timeoutMs);
        }
        this.pending.set(requestId, { onProgress: options.onProgress });
        const result = await invoke<T>("core_request", {
          id: requestId,
          operation,
          payload,
        });
        if (timer) clearTimeout(timer);
        this.pending.delete(requestId);
        resolve(result);
      } catch (error) {
        if (timer) clearTimeout(timer);
        if (id) this.pending.delete(id);
        reject(normalizeCoreError(error));
      }
    });
  }

  async cancel(requestId: string): Promise<boolean> {
    const result = await this.request<{ cancelled?: boolean }>("core.cancel", { id: requestId });
    return Boolean(result.cancelled);
  }

  private initialize(): Promise<void> {
    if (!this.initialized) {
      this.initialized = listen<{
        id?: string;
        current?: number;
        total?: number;
        message?: string;
      }>("core://progress", ({ payload }) => {
        const pending = payload.id ? this.pending.get(payload.id) : undefined;
        if (!pending?.onProgress || payload.current == null || payload.total == null) return;
        pending.onProgress({
          current: payload.current,
          total: payload.total,
          message: payload.message ?? "",
        });
      }).then(() => undefined);
    }
    return this.initialized;
  }
}

function normalizeCoreError(error: unknown): Error {
  if (error && typeof error === "object") {
    const record = error as { message?: unknown; code?: unknown; details?: unknown };
    if (typeof record.message === "string" || record.code != null || record.details != null) {
      const message =
        typeof record.message === "string" && record.message.trim()
          ? record.message
          : formatUnknownError(error);
      return Object.assign(new Error(message), {
        code: record.code,
        details: record.details,
      });
    }
  }
  if (error instanceof Error) {
    if (error.message.trim()) return error;
    return Object.assign(new Error(formatUnknownError(error)), {
      cause: error,
    });
  }
  return new Error(formatUnknownError(error));
}

export function isCancellationError(error: unknown): boolean {
  if (!error || typeof error !== "object") return false;
  const code = (error as { code?: unknown }).code;
  return code === "PLAN_CANCELLED" || code === "CONVERT_CANCELLED";
}

export const core = new CoreClient();

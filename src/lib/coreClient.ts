import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CoreOperation } from "@shared/contracts";

type Progress = { current: number; total: number; message: string };

type PendingRequest = {
  onProgress?: (progress: Progress) => void;
};

class CoreClient {
  private readonly pending = new Map<string, PendingRequest>();
  private initialized?: Promise<void>;

  request<T>(
    operation: CoreOperation,
    payload: unknown,
    timeoutMs = 120_000,
    onProgress?: (progress: Progress) => void,
  ): Promise<T> {
    return new Promise<T>(async (resolve, reject) => {
      let id: string | undefined;
      let timer: ReturnType<typeof setTimeout> | undefined;
      try {
        await this.initialize();
        const requestId = crypto.randomUUID();
        id = requestId;
        timer = setTimeout(() => {
          this.pending.delete(requestId);
          reject(new Error(`轉換核心要求逾時：${operation}`));
        }, timeoutMs);
        this.pending.set(requestId, { onProgress });
        const result = await invoke<T>("core_request", {
          id: requestId,
          operation,
          payload,
        });
        clearTimeout(timer);
        this.pending.delete(requestId);
        resolve(result);
      } catch (error) {
        if (timer) clearTimeout(timer);
        if (id) this.pending.delete(id);
        reject(normalizeCoreError(error));
      }
    });
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
    if (typeof record.message === "string") {
      return Object.assign(new Error(record.message), {
        code: record.code,
        details: record.details,
      });
    }
  }
  return error instanceof Error ? error : new Error(String(error));
}

export const core = new CoreClient();

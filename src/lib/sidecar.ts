import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SidecarOperation, SidecarResponse } from "@shared/contracts";

type PendingRequest = {
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
  timer: ReturnType<typeof setTimeout>;
  onProgress?: (progress: NonNullable<SidecarResponse["progress"]>) => void;
};

class SidecarClient {
  private readonly pending = new Map<string, PendingRequest>();
  private initialized?: Promise<void>;

  request<T>(
    operation: SidecarOperation,
    payload: unknown,
    timeoutMs = 120_000,
    onProgress?: (progress: NonNullable<SidecarResponse["progress"]>) => void,
  ): Promise<T> {
    return new Promise<T>(async (resolve, reject) => {
      let id: string | undefined;
      try {
        await this.initialize();
        const requestId = crypto.randomUUID();
        id = requestId;
        const timer = setTimeout(() => {
          this.pending.delete(requestId);
          reject(new Error(`Sidecar 要求逾時：${operation}`));
        }, timeoutMs);
        this.pending.set(requestId, { resolve: resolve as (value: unknown) => void, reject, timer, onProgress });
        await invoke("sidecar_send", { request: JSON.stringify({ id: requestId, operation, payload }) });
      } catch (error) {
        if (id) {
          const pending = this.pending.get(id);
          if (pending) clearTimeout(pending.timer);
          this.pending.delete(id);
        }
        reject(error);
      }
    });
  }

  private initialize(): Promise<void> {
    if (!this.initialized) {
      this.initialized = Promise.all([
        listen<string>("sidecar://message", ({ payload }) => this.receive(payload)),
        listen<string>("sidecar://error", ({ payload }) => console.error("Sidecar:", payload)),
        listen<number | null>("sidecar://terminated", ({ payload }) => this.failAll(`Sidecar 已終止。結束碼：${payload ?? "未知"}`)),
      ]).then(() => undefined);
    }
    return this.initialized;
  }

  private receive(line: string): void {
    let response: SidecarResponse;
    try {
      response = JSON.parse(line) as SidecarResponse;
    } catch {
      console.error("無法解析 sidecar 回應。", line);
      return;
    }
    const pending = this.pending.get(response.id);
    if (!pending) return;
    if (response.type === "progress") {
      if (response.progress) pending.onProgress?.(response.progress);
      return;
    }
    clearTimeout(pending.timer);
    this.pending.delete(response.id);
    if (response.ok) pending.resolve(response.result);
    else pending.reject(Object.assign(new Error(response.error?.message ?? "Sidecar 操作失敗。"), { code: response.error?.code, details: response.error?.details }));
  }

  private failAll(message: string): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(new Error(message));
    }
    this.pending.clear();
  }
}

export const sidecar = new SidecarClient();

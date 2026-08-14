import { ConvertZZError } from "../errors.js";
import type { Direction, ZhConvertOptions } from "../../../shared/contracts.js";

interface ServiceInfo {
  data?: {
    maxPostBodyBytes?: number;
    converters?: unknown;
    modules?: unknown;
  };
  [key: string]: unknown;
}

export class ZhConvertClient {
  private apiKey = "";
  private serviceInfoCache?: { expiresAt: number; value: ServiceInfo };

  constructor(private readonly baseUrl = "https://api.zhconvert.org") {}

  configure(apiKey: string): void {
    this.apiKey = apiKey.trim();
  }

  async serviceInfo(force = false): Promise<ServiceInfo> {
    if (!force && this.serviceInfoCache && this.serviceInfoCache.expiresAt > Date.now()) {
      return this.serviceInfoCache.value;
    }

    const response = await fetch(`${this.baseUrl}/service-info`, {
      headers: this.apiKey ? { "X-API-Key": this.apiKey } : undefined,
    });
    if (!response.ok) {
      throw new ConvertZZError("ZHCONVERT_SERVICE_INFO", `ZhConvert 服務資訊讀取失敗。HTTP ${response.status}`);
    }
    const value = (await response.json()) as ServiceInfo;
    this.serviceInfoCache = { expiresAt: Date.now() + 86_400_000, value };
    return value;
  }

  async convert(text: string, direction: Direction, options: ZhConvertOptions = {}): Promise<string> {
    if (direction === "none" || !text) return text;
    const info = await this.serviceInfo();
    const maximum = Math.max(1024, Number(info.data?.maxPostBodyBytes ?? 50_000) - 2048);
    const chunks = splitUtf8(text, maximum);
    const converted: string[] = [];

    for (const chunk of chunks) {
      const converter = options.converter ?? (direction === "s2t" ? "Taiwan" : "Simplified");
      const body = new URLSearchParams({
        text: chunk,
        converter,
        outputFormat: "json",
      });
      if (this.apiKey) body.set("apiKey", this.apiKey);
      if (options.modules) {
        const modules = Array.isArray(options.modules)
          ? Object.fromEntries(options.modules.map((name) => [name, 1]))
          : options.modules;
        if (Object.keys(modules).length) body.set("modules", JSON.stringify(modules));
      }
      if (options.jpTextConversionStrategy) body.set("jpTextConversionStrategy", options.jpTextConversionStrategy);
      if (options.jpStyleConversionStrategy) body.set("jpStyleConversionStrategy", options.jpStyleConversionStrategy);
      if (options.cleanUpText !== undefined) body.set("cleanUpText", String(options.cleanUpText));
      if (options.userPreReplace) body.set("userPreReplace", options.userPreReplace);
      if (options.userPostReplace) body.set("userPostReplace", options.userPostReplace);
      if (options.userProtectReplace) body.set("userProtectReplace", options.userProtectReplace);
      if (options.ensureNewlineAtEof !== undefined) body.set("ensureNewlineAtEof", String(options.ensureNewlineAtEof));
      if (options.translateTabsToSpaces !== undefined) body.set("translateTabsToSpaces", String(options.translateTabsToSpaces));
      if (options.trimTrailingWhiteSpaces !== undefined) body.set("trimTrailingWhiteSpaces", String(options.trimTrailingWhiteSpaces));
      if (options.unifyLeadingHyphen !== undefined) body.set("unifyLeadingHyphen", String(options.unifyLeadingHyphen));
      if (options.ignoreTextStyles) body.set("ignoreTextStyles", options.ignoreTextStyles);
      if (options.jpTextStyles) body.set("jpTextStyles", options.jpTextStyles);

      const response = await fetch(`${this.baseUrl}/convert`, {
        method: "POST",
        headers: { "content-type": "application/x-www-form-urlencoded;charset=UTF-8" },
        body,
      });
      if (!response.ok) {
        const detail = await response.text();
        throw new ConvertZZError("ZHCONVERT_CONVERT", `ZhConvert 轉換失敗。HTTP ${response.status}`, detail);
      }
      const payload = (await response.json()) as { data?: { text?: string } | string; text?: string };
      const output = typeof payload.data === "string" ? payload.data : payload.data?.text ?? payload.text;
      if (typeof output !== "string") {
        throw new ConvertZZError("ZHCONVERT_RESPONSE", "ZhConvert 回應不含文字結果。", payload);
      }
      converted.push(output);
    }

    return converted.join("");
  }
}

function splitUtf8(text: string, maximumBytes: number): string[] {
  if (Buffer.byteLength(text, "utf8") <= maximumBytes) return [text];
  const chunks: string[] = [];
  let remaining = text;

  while (remaining) {
    let low = 1;
    let high = remaining.length;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      if (Buffer.byteLength(remaining.slice(0, middle), "utf8") <= maximumBytes) low = middle;
      else high = middle - 1;
    }
    let boundary = low;
    const natural = remaining.slice(0, boundary).search(/[。！？!?\n][^。！？!?\n]*$/u);
    if (natural > boundary / 2) boundary = natural + 1;
    if (/^[\uDC00-\uDFFF]$/u.test(remaining[boundary] ?? "")) boundary -= 1;
    chunks.push(remaining.slice(0, boundary));
    remaining = remaining.slice(boundary);
  }

  return chunks;
}

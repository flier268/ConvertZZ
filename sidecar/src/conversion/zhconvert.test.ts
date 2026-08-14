import { createServer, type Server } from "node:http";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { ZhConvertClient } from "./zhconvert.js";

let server: Server;
let baseUrl = "";
let serviceInfoRequests = 0;
let convertRequests = 0;
let failConversion = false;

beforeAll(async () => {
  server = createServer(async (request, response) => {
    if (request.url === "/service-info") {
      serviceInfoRequests += 1;
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ data: { maxPostBodyBytes: 2048 } }));
      return;
    }
    if (request.url === "/convert" && request.method === "POST") {
      convertRequests += 1;
      if (failConversion) {
        response.statusCode = 503;
        response.end("暫時無法使用");
        return;
      }
      let body = "";
      for await (const chunk of request) body += chunk.toString();
      const text = new URLSearchParams(body).get("text") ?? "";
      response.setHeader("content-type", "application/json");
      response.end(JSON.stringify({ data: { text: text.replaceAll("里", "裡") } }));
      return;
    }
    response.statusCode = 404;
    response.end();
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("模擬伺服器啟動失敗");
  baseUrl = `http://127.0.0.1:${address.port}`;
});

afterEach(() => {
  serviceInfoRequests = 0;
  convertRequests = 0;
  failConversion = false;
});

afterAll(async () => {
  await new Promise<void>((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
});

describe("ZhConvert 客戶端", () => {
  it("快取服務資訊並依限制切分 UTF-8 內容", async () => {
    const client = new ZhConvertClient(baseUrl);
    const source = "里".repeat(1_200);
    expect(await client.convert(source, "s2t")).toBe("裡".repeat(1_200));
    expect(await client.convert("里", "s2t")).toBe("裡");
    expect(serviceInfoRequests).toBe(1);
    expect(convertRequests).toBeGreaterThan(2);
  });

  it("網路服務失敗時回報結構化錯誤", async () => {
    const client = new ZhConvertClient(baseUrl);
    failConversion = true;
    await expect(client.convert("里面", "s2t")).rejects.toMatchObject({ code: "ZHCONVERT_CONVERT" });
  });
});

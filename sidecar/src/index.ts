#!/usr/bin/env node
import { createInterface } from "node:readline";
import type {
  AudioScanRequest,
  AudioTagPlanRequest,
  ConversionRequest,
  FilePlanRequest,
  SidecarRequest,
  SidecarResponse,
  UtilityConvertRequest,
} from "../../shared/contracts.js";
import { AudioService } from "./audio/service.js";
import { parseLegacyCli } from "./cli.js";
import { ConversionService } from "./conversion/engines.js";
import { DictionaryService } from "./dictionary/service.js";
import { toErrorPayload } from "./errors.js";
import { FileService } from "./files/service.js";
import { migrateSettings, migrateSettingsFromPath } from "./settings/migrate.js";
import { convertUtility } from "./utility.js";

const options = parseOptions(process.argv.slice(2));
const conversion = new ConversionService(options.dictionary);
const files = new FileService(conversion);
const audio = new AudioService(conversion, options.wasm);
const dictionary = new DictionaryService(options.dictionary);

process.on("uncaughtException", (error) => console.error("[convertzz-sidecar]", error));
process.on("unhandledRejection", (error) => console.error("[convertzz-sidecar]", error));

const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
input.on("line", async (line) => {
  if (!line.trim()) return;
  let request: SidecarRequest;
  try {
    request = JSON.parse(line) as SidecarRequest;
  } catch (error) {
    send({
      id: "invalid-json",
      type: "response",
      ok: false,
      error: {
        code: "INVALID_JSON",
        message: error instanceof Error ? error.message : String(error),
      },
    });
    return;
  }

  try {
    const result = await dispatch(request, (progress) =>
      send({
        id: request.id,
        type: "progress",
        ok: true,
        progress,
      }),
    );
    send({ id: request.id, type: "response", ok: true, result });
  } catch (error) {
    send({ id: request.id, type: "response", ok: false, error: toErrorPayload(error) });
  }
});

async function dispatch(
  request: SidecarRequest,
  progress: (value: NonNullable<SidecarResponse["progress"]>) => void,
): Promise<unknown> {
  switch (request.operation) {
    case "health":
      return { version: "2.0.0", node: process.versions.node, pid: process.pid };
    case "convert.preview":
      return conversion.convert(request.payload as ConversionRequest);
    case "files.plan":
      return files.plan(request.payload as FilePlanRequest, progress);
    case "files.apply":
      return files.apply(
        (request.payload as { planId: string }).planId,
        progress,
        (request.payload as { selectedPaths?: string[] }).selectedPaths,
      );
    case "files.cancel":
      return files.cancel((request.payload as { planId: string }).planId);
    case "audio.scan":
      return audio.scan(request.payload as AudioScanRequest, progress);
    case "audio.plan":
      return audio.plan(request.payload as AudioTagPlanRequest, progress);
    case "audio.apply":
      return audio.apply((request.payload as { planId: string }).planId, progress);
    case "audio.cancel":
      return audio.cancel((request.payload as { planId: string }).planId);
    case "dictionary.read":
      return dictionary.read(
        request.payload as { path?: string; query?: string; offset?: number; limit?: number },
      );
    case "dictionary.update":
      return dictionary.update(request.payload as Parameters<DictionaryService["update"]>[0]);
    case "dictionary.preview":
      return dictionary.preview(request.payload as Parameters<DictionaryService["preview"]>[0]);
    case "settings.migrate":
      if ((request.payload as { path?: string }).path) {
        return migrateSettingsFromPath((request.payload as { path: string }).path);
      }
      return migrateSettings((request.payload as { input: unknown }).input);
    case "zhconvert.configure":
      conversion.zhconvert.configure((request.payload as { apiKey: string }).apiKey);
      return { configured: true };
    case "zhconvert.serviceInfo":
      return conversion.zhconvert.serviceInfo(
        Boolean((request.payload as { force?: boolean }).force),
      );
    case "utility.convert":
      return { text: convertUtility(request.payload as UtilityConvertRequest) };
    case "cli.parse":
      return parseLegacyCli(
        (request.payload as { args: string[] }).args,
        (request.payload as { defaultEngine?: ConversionRequest["engine"] }).defaultEngine,
      );
  }
}

function send(response: SidecarResponse): void {
  process.stdout.write(`${JSON.stringify(response)}\n`);
}

function parseOptions(args: string[]): { dictionary?: string; wasm?: string } {
  const parsed: { dictionary?: string; wasm?: string } = {};
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === "--dictionary") parsed.dictionary = args[++index];
    else if (args[index] === "--wasm") parsed.wasm = args[++index];
  }
  return parsed;
}

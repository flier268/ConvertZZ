import { randomUUID } from "node:crypto";
import {
  chmod,
  copyFile,
  lstat,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { dirname, extname, join, resolve } from "node:path";
import MP3Tag from "mp3tag.js";
import { TagLib, type PropertyMap } from "taglib-wasm";
import type {
  ApplyResult,
  AudioContainer,
  AudioScanRequest,
  AudioTagField,
  AudioTagFile,
  AudioTagPlan,
  AudioTagPlanRequest,
  TextEncoding,
} from "../../../shared/contracts.js";
import type { ConversionService } from "../conversion/engines.js";
import { decodeText, encodeText } from "../encoding/codecs.js";
import { ConvertZZError } from "../errors.js";

const STANDARD_FIELDS = new Set(["title", "artist", "album", "year", "track", "comment", "genre"]);
const LABELS: Record<string, string> = {
  title: "標題",
  artist: "演出者",
  album: "專輯",
  albumArtist: "專輯演出者",
  comment: "註解",
  genre: "類型",
  composer: "作曲者",
  lyrics: "歌詞",
  year: "年份",
  track: "音軌",
};

interface PreparedAudio {
  path: string;
  format: AudioTagFile["format"];
  updates: Record<string, string[]>;
  selectedContainers: Set<AudioContainer>;
  request: AudioTagPlanRequest;
  originalPictureCount: number;
}

interface StoredAudioPlan {
  publicPlan: AudioTagPlan;
  files: PreparedAudio[];
}

type ProgressReporter = (progress: { current: number; total: number; message: string }) => void;

export class AudioService {
  private taglibPromise?: Promise<TagLib>;
  private readonly plans = new Map<string, StoredAudioPlan>();
  private readonly cancelledPlans = new Set<string>();

  constructor(
    private readonly conversion: ConversionService,
    private readonly wasmPath?: string,
  ) {}

  cancel(planId: string): { cancelled: boolean } {
    const cancelled = this.plans.has(planId);
    if (cancelled) {
      this.cancelledPlans.add(planId);
      this.plans.delete(planId);
    }
    return { cancelled };
  }

  async scan(request: AudioScanRequest, report?: ProgressReporter): Promise<AudioTagFile[]> {
    const paths = await expandAudioPaths(request.paths, request.recursive ?? false);
    const files: AudioTagFile[] = [];
    for (const [index, path] of paths.entries()) {
      try {
        files.push(await this.scanFile(resolve(path), request));
      } catch (error) {
        files.push({
          path: resolve(path),
          format: formatFromPath(path),
          selected: true,
          fields: [],
          hasCoverArt: false,
          warning: error instanceof Error ? error.message : String(error),
        });
      }
      report?.({
        current: index + 1,
        total: paths.length,
        message: `正在掃描：${path.split(/[\\/]/u).at(-1) ?? path}`,
      });
    }
    return files;
  }

  async plan(request: AudioTagPlanRequest, report?: ProgressReporter): Promise<AudioTagPlan> {
    const scanned = await this.scan(request);
    const prepared: PreparedAudio[] = [];
    const warnings: string[] = [];
    const selectedPaths = new Set(request.selectedPaths.map((path) => resolve(path)));

    for (const [index, file] of scanned.entries()) {
      file.selected = selectedPaths.has(file.path);
      if (file.warning) continue;
      if (!file.selected) {
        file.fields.forEach((field) => {
          field.selected = false;
        });
        report?.({
          current: index + 1,
          total: scanned.length,
          message: `已略過未選檔案：${file.path.split(/[\\/]/u).at(-1) ?? file.path}`,
        });
        continue;
      }
      const selected = new Set(request.selectedFields[file.path] ?? []);
      const updates: Record<string, string[]> = {};
      const containers = new Set<AudioContainer>();

      for (const field of file.fields) {
        const identifier = fieldId(field.container, field.key);
        field.selected = selected.has(identifier) && containerEnabled(request, field.container);
        if (!field.selected) continue;
        const convertedValues: string[] = [];
        for (const value of field.values) {
          const result = await this.conversion.convert({
            text: value,
            ...conversionForContainer(request, field.container),
          });
          convertedValues.push(result.text);
          warnings.push(...result.warnings);
        }
        field.convertedValues = convertedValues;
        updates[identifier] = convertedValues;
        containers.add(field.container);
      }

      prepared.push({
        path: file.path,
        format: file.format,
        updates,
        selectedContainers: containers,
        request,
        originalPictureCount: file.hasCoverArt ? await this.pictureCount(file.path) : 0,
      });
      report?.({
        current: index + 1,
        total: scanned.length,
        message: `正在建立標籤預覽：${file.path.split(/[\\/]/u).at(-1) ?? file.path}`,
      });
    }

    const planId = randomUUID();
    const publicPlan: AudioTagPlan = {
      planId,
      createdAt: new Date().toISOString(),
      files: scanned,
      warnings: Array.from(new Set(warnings)),
    };
    this.plans.set(planId, { publicPlan, files: prepared });
    return publicPlan;
  }

  async apply(planId: string, report?: ProgressReporter): Promise<ApplyResult> {
    const plan = this.plans.get(planId);
    if (!plan) throw new ConvertZZError("PLAN_NOT_FOUND", "音訊標籤計畫已失效。請重新預覽。");
    const result: ApplyResult = { succeeded: [], skipped: [], failed: [] };

    for (const [index, file] of plan.files.entries()) {
      this.throwIfCancelled(planId);
      if (Object.keys(file.updates).length === 0) {
        result.skipped.push(file.path);
        continue;
      }
      try {
        if (file.format === "mp3") await this.applyMp3(file);
        else await this.applyTagLib(file);
        result.succeeded.push(file.path);
      } catch (error) {
        result.failed.push({
          path: file.path,
          message: error instanceof Error ? error.message : String(error),
        });
      }
      report?.({
        current: index + 1,
        total: plan.files.length,
        message: `正在寫入標籤：${file.path.split(/[\\/]/u).at(-1) ?? file.path}`,
      });
    }

    this.plans.delete(planId);
    this.cancelledPlans.delete(planId);
    return result;
  }

  private async scanFile(path: string, request: AudioScanRequest): Promise<AudioTagFile> {
    const format = formatFromPath(path);
    if (format === "mp3") {
      return this.scanMp3(
        path,
        request.id3v1SourceEncoding ?? "gbk",
        request.id3v2SourceEncoding ?? "gbk",
        request.id3v2RepairSourceEncoding ?? false,
      );
    }
    const taglib = await this.taglib();
    const file = await taglib.open(path);
    try {
      if (!file.isValid()) throw new ConvertZZError("AUDIO_INVALID", "音訊檔案無法解析。");
      const container: AudioContainer = format === "ape" ? "apev2" : "vorbis-comment";
      const fields = Object.entries(file.properties()).flatMap(([key, values]) =>
        values?.every((value) => typeof value === "string")
          ? [makeField(container, key, values)]
          : [],
      );
      const properties = file.audioProperties();
      return {
        path,
        format,
        selected: true,
        fields,
        hasCoverArt: file.getPictures().length > 0,
        durationSeconds: properties?.duration,
      };
    } finally {
      file.dispose();
    }
  }

  private async scanMp3(
    path: string,
    id3v1Encoding: Exclude<TextEncoding, "auto">,
    id3v2Encoding: Exclude<TextEncoding, "auto">,
    repairId3v2: boolean,
  ): Promise<AudioTagFile> {
    const buffer = await readFile(path);
    const parser = new MP3Tag(buffer);
    const tags = parser.read({ id3v1: true, id3v2: true, unsupported: true });
    if (parser.error) throw new ConvertZZError("ID3_READ", parser.error);
    const fields: AudioTagField[] = [];
    const v1 = readId3v1(buffer, id3v1Encoding);
    if (v1) {
      for (const [key, value] of Object.entries(v1.values))
        fields.push(makeField("id3v1", key, [value]));
    }
    if (tags.v2) {
      for (const key of [
        "title",
        "artist",
        "album",
        "year",
        "track",
        "comment",
        "genre",
      ] as const) {
        const value = tags[key];
        if (typeof value === "string")
          fields.push(
            makeField("id3v2", key, [repairId3v2Value(value, id3v2Encoding, repairId3v2)]),
          );
      }
      fields.push(
        ...readAdditionalId3v2TextFields(tags.v2).map((field) => ({
          ...field,
          values: field.values.map((value) => repairId3v2Value(value, id3v2Encoding, repairId3v2)),
        })),
      );
    }
    const pictures = tags.v2?.APIC ?? tags.v2?.PIC ?? [];
    return { path, format: "mp3", selected: true, fields, hasCoverArt: pictures.length > 0 };
  }

  private async applyMp3(file: PreparedAudio): Promise<void> {
    const source = await readFile(file.path);
    const parser = new MP3Tag(source);
    const tags = parser.read({ id3v1: true, id3v2: true, unsupported: true });
    if (parser.error) throw new ConvertZZError("ID3_READ", parser.error);

    const writesId3v1 = file.selectedContainers.has("id3v1");
    const writesId3v2 = file.selectedContainers.has("id3v2");
    if (writesId3v2) {
      for (const [identifier, values] of Object.entries(file.updates)) {
        const [container, key] = splitFieldId(identifier);
        if (container !== "id3v2") continue;
        if (key in tags) (tags as unknown as Record<string, unknown>)[key] = values[0] ?? "";
        else updateAdditionalId3v2TextField(tags.v2, key, values);
      }
      parser.tags = tags;
    }

    let output: Buffer<ArrayBufferLike> = writesId3v2
      ? Buffer.from(
          parser.save({
            strict: false,
            id3v1: { include: false },
            id3v2: {
              include: true,
              version: file.request.id3v2Version,
              encoding: file.request.id3v2Encoding,
              unsupported: true,
            },
          }) as ArrayBuffer,
        )
      : Buffer.from(stripId3v1(source));
    if (parser.error) throw new ConvertZZError("ID3_WRITE", parser.error);

    const existingV1 = readId3v1(source, file.request.id3v1SourceEncoding ?? "gbk");
    if (writesId3v1) {
      const values = existingV1?.values ?? emptyId3v1();
      for (const [identifier, converted] of Object.entries(file.updates)) {
        const [container, key] = splitFieldId(identifier);
        if (container === "id3v1" && key in values)
          values[key as keyof typeof values] = converted[0] ?? "";
      }
      output = appendId3v1(
        stripId3v1(output),
        values,
        existingV1?.genreCode ?? 255,
        file.request.id3v1OutputEncoding,
      );
    } else if (existingV1) {
      output = Buffer.concat([stripId3v1(output), source.subarray(source.length - 128)]);
    }
    await this.replaceMp3Atomically(file, source, output);
  }

  private async replaceMp3Atomically(
    file: PreparedAudio,
    source: Buffer,
    content: Buffer,
  ): Promise<void> {
    const temporary = temporaryPath(file.path);
    try {
      await writeFile(temporary, content, { flag: "wx" });
      const sourceStat = await stat(file.path);
      await chmod(temporary, sourceStat.mode);

      const staged = await readFile(temporary);
      const verifiedParser = new MP3Tag(staged);
      const verifiedTags = verifiedParser.read({ id3v1: true, id3v2: true, unsupported: true });
      if (verifiedParser.error)
        throw new ConvertZZError(
          "AUDIO_VERIFY",
          `MP3 標籤寫入後無法重新解析：${verifiedParser.error}`,
        );

      if (!mp3AudioPayload(source).equals(mp3AudioPayload(staged))) {
        throw new ConvertZZError("AUDIO_VERIFY", "MP3 標籤寫入改變了音訊資料。");
      }

      const sourceParser = new MP3Tag(source);
      const sourceTags = sourceParser.read({ id3v1: true, id3v2: true, unsupported: true });
      if (sourceParser.error) throw new ConvertZZError("ID3_READ", sourceParser.error);
      if (pictureFingerprint(sourceTags.v2) !== pictureFingerprint(verifiedTags.v2)) {
        throw new ConvertZZError("AUDIO_PICTURE", "MP3 標籤寫入造成封面圖片改變。");
      }

      const verified = await this.scanMp3(
        temporary,
        file.selectedContainers.has("id3v1")
          ? file.request.id3v1OutputEncoding
          : (file.request.id3v1SourceEncoding ?? "gbk"),
        "utf8",
        false,
      );
      const expectedV1 = readId3v1(content, file.request.id3v1OutputEncoding)?.values;
      for (const [identifier, expected] of Object.entries(file.updates)) {
        const [container, key] = splitFieldId(identifier);
        const field = verified.fields.find(
          (candidate) => candidate.container === container && candidate.key === key,
        );
        const expectedValues =
          container === "id3v1" && expectedV1 && key in expectedV1
            ? [expectedV1[key as keyof Id3v1Values]]
            : expected;
        if (!field || !sameValues(field.values, expectedValues)) {
          throw new ConvertZZError("AUDIO_VERIFY", `MP3 標籤欄位 ${identifier} 寫入後驗證失敗。`);
        }
      }

      await commitTemporary(file.path, temporary);
    } catch (error) {
      await rm(temporary, { force: true });
      throw error;
    }
  }

  private async applyTagLib(file: PreparedAudio): Promise<void> {
    const temporary = temporaryPath(file.path);
    await copyFile(file.path, temporary);
    try {
      const taglib = await this.taglib();
      await taglib.edit(temporary, (audio) => {
        const properties: PropertyMap = { ...audio.properties() };
        for (const [identifier, values] of Object.entries(file.updates)) {
          const [, key] = splitFieldId(identifier);
          properties[key] = values;
        }
        audio.setProperties(properties);
      });
      const verification = await taglib.open(temporary);
      try {
        if (!verification.isValid())
          throw new ConvertZZError("AUDIO_VERIFY", "標籤寫入後的音訊檔案無法驗證。");
        if (verification.getPictures().length !== file.originalPictureCount) {
          throw new ConvertZZError("AUDIO_PICTURE", "標籤寫入造成封面圖片數量改變。");
        }
      } finally {
        verification.dispose();
      }
      await commitTemporary(file.path, temporary);
    } catch (error) {
      await rm(temporary, { force: true });
      throw error;
    }
  }

  private async pictureCount(path: string): Promise<number> {
    if (formatFromPath(path) === "mp3") return 0;
    const taglib = await this.taglib();
    const file = await taglib.open(path);
    try {
      return file.getPictures().length;
    } finally {
      file.dispose();
    }
  }

  private taglib(): Promise<TagLib> {
    if (!this.taglibPromise) {
      this.taglibPromise = this.wasmPath
        ? TagLib.initialize({ wasmUrl: this.wasmPath, forceWasmType: "wasi" })
        : TagLib.initialize();
    }
    return this.taglibPromise;
  }

  private throwIfCancelled(planId: string): void {
    if (this.cancelledPlans.has(planId))
      throw new ConvertZZError("PLAN_CANCELLED", "音訊標籤作業已由使用者取消。");
  }
}

async function expandAudioPaths(inputPaths: string[], recursive: boolean): Promise<string[]> {
  const paths = new Set<string>();
  for (const inputPath of inputPaths) {
    const path = resolve(inputPath);
    const metadata = await lstat(path);
    if (metadata.isSymbolicLink()) continue;
    if (metadata.isFile()) {
      formatFromPath(path);
      paths.add(path);
      continue;
    }
    if (metadata.isDirectory()) await collectAudioFiles(path, recursive, paths);
  }
  return Array.from(paths).sort((left, right) => left.localeCompare(right));
}

async function collectAudioFiles(
  directory: string,
  recursive: boolean,
  paths: Set<string>,
): Promise<void> {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.isSymbolicLink()) continue;
    const path = join(directory, entry.name);
    if (entry.isFile() && isSupportedAudioPath(path)) paths.add(resolve(path));
    else if (entry.isDirectory() && recursive) await collectAudioFiles(path, true, paths);
  }
}

function isSupportedAudioPath(path: string): boolean {
  return [".mp3", ".ape", ".ogg", ".oga", ".opus"].includes(extname(path).toLowerCase());
}

function containerEnabled(request: AudioTagPlanRequest, container: AudioContainer): boolean {
  if (container === "id3v1") return request.id3v1Enabled;
  if (container === "id3v2") return request.id3v2Enabled;
  return true;
}

function directionForContainer(
  request: AudioTagPlanRequest,
  container: AudioContainer,
): AudioTagPlanRequest["conversion"]["direction"] {
  if (container === "id3v1") return request.id3v1Direction;
  if (container === "id3v2") return request.id3v2Direction;
  return request.conversion.direction;
}

function conversionForContainer(
  request: AudioTagPlanRequest,
  container: AudioContainer,
): AudioTagPlanRequest["conversion"] {
  const zhconvert =
    container === "id3v1"
      ? (request.id3v1Zhconvert ?? request.conversion.zhconvert)
      : container === "id3v2"
        ? (request.id3v2Zhconvert ?? request.conversion.zhconvert)
        : request.conversion.zhconvert;
  return { ...request.conversion, direction: directionForContainer(request, container), zhconvert };
}

const LATIN1_REPAIR_CHARACTERS = new Set(
  Array.from(
    "¡¢£¤¥¦§¨©ª«¬®¯°±²³´µ¶·¸¹º»¼½¾¿ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿ",
  ),
);

function repairId3v2Value(
  value: string,
  encoding: Exclude<TextEncoding, "auto">,
  enabled: boolean,
): string {
  if (!enabled || value.length === 0) return value;
  const characters = Array.from(value);
  const latin1Count = characters.filter((character) =>
    LATIN1_REPAIR_CHARACTERS.has(character),
  ).length;
  if (latin1Count / characters.length <= 0.2) return value;
  return decodeText(Buffer.from(value, "latin1"), encoding).text;
}

function mp3AudioPayload(buffer: Buffer): Buffer {
  return Buffer.from(new MP3Tag(buffer).getAudio(true) as ArrayBuffer);
}

function pictureFingerprint(v2: unknown): string {
  if (!isRecord(v2)) return "[]";
  const pictures = [v2.APIC, v2.PIC].flatMap((value) =>
    Array.isArray(value) ? value : value ? [value] : [],
  );
  return JSON.stringify(normalizeFingerprint(pictures));
}

function normalizeFingerprint(value: unknown): unknown {
  if (value instanceof ArrayBuffer) return Array.from(new Uint8Array(value));
  if (ArrayBuffer.isView(value)) {
    return Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
  }
  if (Array.isArray(value)) return value.map(normalizeFingerprint);
  if (isRecord(value)) {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, normalizeFingerprint(value[key])]),
    );
  }
  return value;
}

function sameValues(actual: string[], expected: string[]): boolean {
  return (
    actual.length === expected.length && actual.every((value, index) => value === expected[index])
  );
}

function makeField(container: AudioContainer, key: string, values: string[]): AudioTagField {
  return {
    key,
    label: LABELS[key] ?? key,
    container,
    values,
    selected: STANDARD_FIELDS.has(key),
  };
}

const COMMON_ID3V2_FRAMES = new Set([
  "TT2",
  "TIT2",
  "TP1",
  "TPE1",
  "TAL",
  "TALB",
  "TYE",
  "TYER",
  "TDRC",
  "TRK",
  "TRCK",
  "TCO",
  "TCON",
]);

function readAdditionalId3v2TextFields(v2: unknown): AudioTagField[] {
  const frames = v2 as Record<string, unknown>;
  const fields: AudioTagField[] = [];
  for (const [frameId, value] of Object.entries(frames)) {
    if (COMMON_ID3V2_FRAMES.has(frameId)) continue;
    if (/^T[A-Z0-9]{2,3}$/u.test(frameId) && typeof value === "string") {
      fields.push(customMp3Field(`frame:${frameId}`, frameId, [value]));
      continue;
    }
    if ((frameId === "TXXX" || frameId === "TXX") && Array.isArray(value)) {
      value.forEach((frame, index) => {
        if (!isRecord(frame) || typeof frame.text !== "string") return;
        const description =
          typeof frame.description === "string" && frame.description
            ? frame.description
            : `自訂文字 ${index + 1}`;
        fields.push(customMp3Field(`custom:${frameId}:${index}`, description, [frame.text]));
      });
      continue;
    }
    if (
      (frameId === "USLT" || frameId === "ULT" || frameId === "COMM" || frameId === "COM") &&
      Array.isArray(value)
    ) {
      value.forEach((frame, index) => {
        if (!isRecord(frame) || typeof frame.text !== "string") return;
        const description =
          typeof frame.descriptor === "string" && frame.descriptor ? frame.descriptor : frameId;
        fields.push(customMp3Field(`described:${frameId}:${index}`, description, [frame.text]));
      });
    }
  }
  return fields;
}

function customMp3Field(key: string, label: string, values: string[]): AudioTagField {
  return { ...makeField("id3v2", key, values), label, selected: false };
}

function updateAdditionalId3v2TextField(v2: unknown, key: string, values: string[]): void {
  const frames = v2 as Record<string, unknown>;
  const [kind, frameId, rawIndex] = key.split(":");
  if (kind === "frame" && frameId && typeof frames[frameId] === "string") {
    frames[frameId] = values[0] ?? "";
    return;
  }
  if ((kind === "custom" || kind === "described") && frameId && rawIndex) {
    const frame = Array.isArray(frames[frameId]) ? frames[frameId][Number(rawIndex)] : undefined;
    if (isRecord(frame) && typeof frame.text === "string") frame.text = values[0] ?? "";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function fieldId(container: AudioContainer, key: string): string {
  return `${container}:${key}`;
}

function splitFieldId(identifier: string): [AudioContainer, string] {
  const separator = identifier.indexOf(":");
  return [identifier.slice(0, separator) as AudioContainer, identifier.slice(separator + 1)];
}

function formatFromPath(path: string): AudioTagFile["format"] {
  const extension = extname(path).toLowerCase();
  if (extension === ".mp3") return "mp3";
  if (extension === ".ape") return "ape";
  if (extension === ".opus") return "opus";
  if (extension === ".ogg" || extension === ".oga") return "ogg";
  throw new ConvertZZError("AUDIO_FORMAT", `不支援音訊格式 ${extension || "未知"}。`);
}

interface Id3v1Values {
  title: string;
  artist: string;
  album: string;
  year: string;
  comment: string;
  track: string;
  genre: string;
}

function emptyId3v1(): Id3v1Values {
  return { title: "", artist: "", album: "", year: "", comment: "", track: "", genre: "" };
}

function readId3v1(
  buffer: Buffer,
  encoding: Exclude<TextEncoding, "auto">,
): { values: Id3v1Values; genreCode: number } | undefined {
  if (
    buffer.length < 128 ||
    buffer.subarray(buffer.length - 128, buffer.length - 125).toString("ascii") !== "TAG"
  )
    return undefined;
  const tag = buffer.subarray(buffer.length - 128);
  const decode = (start: number, length: number) =>
    decodeText(tag.subarray(start, start + length), encoding).text.replace(/[\u0000 ]+$/g, "");
  const track = tag[125] === 0 && tag[126] > 0 ? String(tag[126]) : "";
  return {
    values: {
      title: decode(3, 30),
      artist: decode(33, 30),
      album: decode(63, 30),
      year: decode(93, 4),
      comment: decode(97, track ? 28 : 30),
      track,
      genre: String(tag[127]),
    },
    genreCode: tag[127],
  };
}

function appendId3v1(
  buffer: Buffer,
  values: Id3v1Values,
  genreCode: number,
  encoding: Exclude<TextEncoding, "auto">,
): Buffer {
  const tag = Buffer.alloc(128);
  tag.write("TAG", 0, "ascii");
  writeEncodedField(tag, 3, 30, values.title, encoding);
  writeEncodedField(tag, 33, 30, values.artist, encoding);
  writeEncodedField(tag, 63, 30, values.album, encoding);
  writeEncodedField(tag, 93, 4, values.year, encoding);
  const track = Number(values.track) || 0;
  writeEncodedField(tag, 97, track ? 28 : 30, values.comment, encoding);
  if (track) {
    tag[125] = 0;
    tag[126] = Math.min(255, track);
  }
  tag[127] = Number(values.genre) || genreCode;
  return Buffer.concat([buffer, tag]);
}

function writeEncodedField(
  target: Buffer,
  offset: number,
  length: number,
  value: string,
  encoding: Exclude<TextEncoding, "auto">,
): void {
  const encoded = encodeText(value, encoding);
  encoded.copy(target, offset, 0, Math.min(length, encoded.length));
}

function stripId3v1(buffer: Buffer): Buffer {
  return buffer.length >= 128 &&
    buffer.subarray(buffer.length - 128, buffer.length - 125).toString("ascii") === "TAG"
    ? buffer.subarray(0, buffer.length - 128)
    : buffer;
}

async function commitTemporary(path: string, temporary: string): Promise<void> {
  const backup = join(dirname(path), `.convertzz-audio-backup-${randomUUID()}${extname(path)}`);
  await rename(path, backup);
  try {
    await rename(temporary, path);
  } catch (error) {
    try {
      await rename(backup, path);
    } catch {
      // Keep the recoverable backup when another process blocks restoration.
    }
    throw error;
  }
  await rm(backup);
}

function temporaryPath(path: string): string {
  return join(dirname(path), `.convertzz-audio-${randomUUID()}${extname(path)}`);
}

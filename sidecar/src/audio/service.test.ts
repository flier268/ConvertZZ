import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { copyFileSync, existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import MP3Tag from "mp3tag.js";
import { TagLib, type Picture, type PropertyMap } from "taglib-wasm";
import type { AudioContainer, AudioTagPlanRequest } from "../../../shared/contracts.js";
import { ConversionService } from "../conversion/engines.js";
import { encodeText } from "../encoding/codecs.js";
import { AudioService } from "./service.js";

const require = createRequire(import.meta.url);
const wasmPath = resolve("node_modules/taglib-wasm/dist/taglib-wasi.wasm");
const ffmpegPath = resolveFfmpeg();
const bundledApe = resolve("tests/fixtures/mac-399.ape");
const bundledOgg = resolve("tests/fixtures/test.ogg");
const apeFixture =
  process.env.CONVERTZZ_APE_FIXTURE ?? (existsSync(bundledApe) ? bundledApe : undefined);
const oggFixture = existsSync(bundledOgg) ? bundledOgg : undefined;
const temporaryDirectories: string[] = [];

afterAll(async () => {
  await Promise.all(temporaryDirectories.map((path) => rm(path, { recursive: true, force: true })));
});

describe("損毀音訊", () => {
  it("回報可辨識錯誤", async () => {
    const directory = await temporaryDirectory("convertzz-audio-invalid-");
    const path = join(directory, "truncated.ogg");
    await writeFile(path, Buffer.from("OggS\u0000truncated"));
    const service = new AudioService(
      new ConversionService(resolve("ConvertZZ/Dictionary.csv")),
      wasmPath,
    );
    const [file] = await service.scan({ paths: [path] });
    expect(file.warning).toBeTruthy();
    expect(file.fields).toEqual([]);
  });
});

describe("音訊批次選取", () => {
  it("依遞迴選項展開資料夾", async () => {
    const directory = await temporaryDirectory("convertzz-audio-folder-");
    const nested = join(directory, "nested");
    await mkdir(nested);
    await writeTaggedMp3(join(directory, "top.mp3"), "里面", "裡面");
    await writeTaggedMp3(join(nested, "child.mp3"), "里面", "裡面");
    await writeFile(join(nested, "ignored.txt"), "not audio");
    const service = audioService();

    const shallow = await service.scan({ paths: [directory], recursive: false });
    expect(shallow.map((file) => file.path)).toEqual([join(directory, "top.mp3")]);

    const recursive = await service.scan({ paths: [directory], recursive: true });
    expect(recursive.map((file) => file.path)).toEqual([
      join(directory, "nested", "child.mp3"),
      join(directory, "top.mp3"),
    ]);
    expect(recursive.every((file) => file.selected)).toBe(true);
  });

  it("只將逐檔勾選的檔案加入寫入計畫", async () => {
    const directory = await temporaryDirectory("convertzz-audio-selection-");
    const selectedPath = join(directory, "selected.mp3");
    const skippedPath = join(directory, "skipped.mp3");
    await writeTaggedMp3(selectedPath, "里面", "裡面");
    await writeTaggedMp3(skippedPath, "里面", "裡面");
    const service = audioService();
    const scanned = await service.scan({ paths: [directory], recursive: false });
    const selectedTitle = scanned
      .find((file) => file.path === selectedPath)!
      .fields.find((field) => field.container === "id3v1" && field.key === "title")!;
    const skippedBefore = await readFile(skippedPath);

    const plan = await service.plan({
      ...basePlanRequest([directory]),
      recursive: false,
      selectedPaths: [selectedPath],
      selectedFields: {
        [selectedPath]: [`id3v1:${selectedTitle.key}`],
        [skippedPath]: ["id3v1:title"],
      },
    });
    expect(plan.files.find((file) => file.path === selectedPath)?.selected).toBe(true);
    expect(plan.files.find((file) => file.path === skippedPath)?.selected).toBe(false);

    const result = await service.apply(plan.planId);
    expect(result.succeeded).toEqual([selectedPath]);
    expect(await readFile(skippedPath)).toEqual(skippedBefore);
  });
});

describe("MP3 容器選項", () => {
  it("分別套用 ID3v1 與 ID3v2 的方向並重新解析暫存結果", async () => {
    const directory = await temporaryDirectory("convertzz-audio-directions-");
    const path = join(directory, "directions.mp3");
    await writeTaggedMp3(path, "里面", "裡面", true);
    const beforeAudio = mp3AudioPayload(await readFile(path));
    const beforePicture = mp3PictureData(await readFile(path));
    const service = audioService();

    const plan = await service.plan({
      ...basePlanRequest([path]),
      selectedPaths: [path],
      selectedFields: { [path]: ["id3v1:title", "id3v2:title"] },
      id3v1Direction: "s2t",
      id3v2Direction: "t2s",
    });
    const preview = plan.files[0];
    expect(
      preview.fields.find((field) => field.container === "id3v1" && field.key === "title")
        ?.convertedValues,
    ).toEqual(["裡面"]);
    expect(
      preview.fields.find((field) => field.container === "id3v2" && field.key === "title")
        ?.convertedValues,
    ).toEqual(["里面"]);

    const result = await service.apply(plan.planId);
    expect(result.failed).toEqual([]);
    const [verified] = await service.scan({ paths: [path], id3v1SourceEncoding: "big5" });
    expect(
      verified.fields.find((field) => field.container === "id3v1" && field.key === "title")?.values,
    ).toEqual(["裡面"]);
    expect(
      verified.fields.find((field) => field.container === "id3v2" && field.key === "title")?.values,
    ).toEqual(["里面"]);
    expect(mp3AudioPayload(await readFile(path))).toEqual(beforeAudio);
    expect(mp3PictureData(await readFile(path))).toEqual(beforePicture);
  });

  it("停用的 ID3 容器不會被轉換", async () => {
    const directory = await temporaryDirectory("convertzz-audio-disabled-");
    const path = join(directory, "disabled.mp3");
    await writeTaggedMp3(path, "里面", "里面");
    const service = audioService();
    const plan = await service.plan({
      ...basePlanRequest([path]),
      selectedPaths: [path],
      selectedFields: { [path]: ["id3v1:title", "id3v2:title"] },
      id3v2Enabled: false,
    });
    expect(
      plan.files[0].fields.find((field) => field.container === "id3v2" && field.key === "title")
        ?.selected,
    ).toBe(false);

    await service.apply(plan.planId);
    const [verified] = await service.scan({ paths: [path], id3v1SourceEncoding: "big5" });
    expect(
      verified.fields.find((field) => field.container === "id3v1" && field.key === "title")?.values,
    ).toEqual(["裡面"]);
    expect(
      verified.fields.find((field) => field.container === "id3v2" && field.key === "title")?.values,
    ).toEqual(["里面"]);
  });

  it("依舊版比例規則修復 ID3v2 來源錯碼", async () => {
    const directory = await temporaryDirectory("convertzz-audio-mojibake-");
    const path = join(directory, "mojibake.mp3");
    const mojibake = encodeText("裡面", "big5").toString("latin1");
    await writeTaggedMp3(path, "里面", mojibake);
    const service = audioService();

    const [raw] = await service.scan({
      paths: [path],
      id3v2SourceEncoding: "big5",
      id3v2RepairSourceEncoding: false,
    });
    expect(
      raw.fields.find((field) => field.container === "id3v2" && field.key === "title")?.values,
    ).toEqual([mojibake]);

    const [repaired] = await service.scan({
      paths: [path],
      id3v2SourceEncoding: "big5",
      id3v2RepairSourceEncoding: true,
    });
    expect(
      repaired.fields.find((field) => field.container === "id3v2" && field.key === "title")?.values,
    ).toEqual(["裡面"]);
  });

  it("ID3v1 可用 Big5 與 GBK 讀回損壞標題", async () => {
    const directory = await temporaryDirectory("convertzz-audio-id3v1-");
    const service = audioService();
    for (const encoding of ["big5", "gbk"] as const) {
      const path = join(directory, `${encoding}.mp3`);
      await writeFile(path, id3v1OnlyMp3("裡面", encoding));
      const [scanned] = await service.scan({ paths: [path], id3v1SourceEncoding: encoding });
      expect(
        scanned.fields.find((field) => field.container === "id3v1" && field.key === "title")
          ?.values,
      ).toEqual(["裡面"]);
    }
  });

  it("可把 ID3v2.4 寫成 2.3／2.4 並套用指定文字編碼", async () => {
    const directory = await temporaryDirectory("convertzz-audio-id3v2-version-");
    const service = audioService();
    const cases = [
      { version: 3 as const, encoding: "utf16" as const, encodingByte: 1 },
      { version: 4 as const, encoding: "utf8" as const, encodingByte: 3 },
      { version: 4 as const, encoding: "utf16" as const, encodingByte: 1 },
    ];
    for (const sample of cases) {
      const path = join(directory, `v${sample.version}-${sample.encoding}.mp3`);
      await writeTaggedMp3(path, "里面", "里面");
      expect(id3v2HeaderVersion(await readFile(path))).toBe(4);
      const plan = await service.plan({
        ...basePlanRequest([path]),
        selectedPaths: [path],
        selectedFields: { [path]: ["id3v2:title"] },
        id3v2Version: sample.version,
        id3v2Encoding: sample.encoding,
      });
      const result = await service.apply(plan.planId);
      expect(result.failed).toEqual([]);
      const written = await readFile(path);
      expect(id3v2HeaderVersion(written)).toBe(sample.version);
      expect(id3v2FrameTextEncoding(written, "TIT2")).toBe(sample.encodingByte);
      const [verified] = await service.scan({ paths: [path] });
      expect(
        verified.fields.find((field) => field.container === "id3v2" && field.key === "title")
          ?.values,
      ).toEqual(["裡面"]);
    }
  });
});

describe("音訊標籤整合", () => {
  const samples: Array<{
    extension: string;
    codec?: string;
    container: AudioContainer;
    fixture?: string;
  }> = [
    { extension: "mp3", codec: "libmp3lame", container: "id3v2" },
    { extension: "ape", fixture: apeFixture, container: "apev2" },
    { extension: "ogg", codec: "libvorbis", container: "vorbis-comment" },
    { extension: "oga", fixture: oggFixture, container: "vorbis-comment" },
    { extension: "opus", codec: "libopus", container: "vorbis-comment" },
  ];
  let taglib: TagLib;
  let directory = "";

  beforeAll(async () => {
    if (!ffmpegPath) return;
    directory = await temporaryDirectory("convertzz-audio-");
    taglib = await TagLib.initialize({ wasmUrl: wasmPath, forceWasmType: "wasi" });
  });

  for (const sample of samples) {
    const needsFixture = sample.extension === "ape" || sample.extension === "oga";
    const available =
      Boolean(ffmpegPath) &&
      (!needsFixture || Boolean(sample.fixture && existsSync(sample.fixture)));
    it.runIf(available)(
      `轉換 .${sample.extension} 標籤並保持音訊與未選欄位`,
      async () => {
        const path = join(directory, `sample.${sample.extension}`);
        generateAudio(path, sample.codec, sample.fixture);
        await taglib.edit(path, (audio) => {
          const properties: PropertyMap = {
            title: ["里面开发"],
            artist: ["头发", "皇后"],
            album: ["未选择里面"],
            CUSTOM_TEXT: ["自订里面"],
          };
          audio.setProperties(properties);
          audio.addPicture(coverPicture());
        });

        const beforeAudio = await audioFingerprint(path, sample.extension);
        const before = await readTagSnapshot(taglib, path);
        const service = new AudioService(
          new ConversionService(resolve("ConvertZZ/Dictionary.csv")),
          wasmPath,
        );
        const [scan] = await service.scan({ paths: [path], id3v1SourceEncoding: "gbk" });
        expect(scan.warning).toBeUndefined();
        expect(scan.hasCoverArt).toBe(true);
        const title = scan.fields.find(
          (field) => field.container === sample.container && field.key.toLowerCase() === "title",
        );
        expect(title).toBeDefined();

        const plan = await service.plan({
          paths: [path],
          selectedPaths: [path],
          selectedFields: { [path]: [`${sample.container}:${title!.key}`] },
          conversion: { direction: "s2t", engine: "segmented" },
          conflictPolicy: "skip",
          id3v1Enabled: true,
          id3v1Direction: "s2t",
          id3v1SourceEncoding: "gbk",
          id3v1OutputEncoding: "big5",
          id3v2Enabled: true,
          id3v2Direction: "s2t",
          id3v2Version: 4,
          id3v2Encoding: "utf8",
        });
        const result = await service.apply(plan.planId);
        expect(result.failed).toEqual([]);
        expect(result.succeeded).toEqual([path]);

        const after = await readTagSnapshot(taglib, path);
        expect(property(after.properties, "title")).toEqual(["裡面開發"]);
        expect(property(after.properties, "artist")).toEqual(property(before.properties, "artist"));
        expect(property(after.properties, "album")).toEqual(property(before.properties, "album"));
        expect(property(after.properties, "custom_text")).toEqual(
          property(before.properties, "custom_text"),
        );
        expect(after.pictures).toEqual(before.pictures);
        expect(await audioFingerprint(path, sample.extension)).toBe(beforeAudio);
      },
      30_000,
    );
  }

  it.runIf(Boolean(ffmpegPath) && Boolean(apeFixture) && Boolean(oggFixture))(
    "依副檔名辨識 MP3、APE、OGG 與 Opus",
    async () => {
      directory ||= await temporaryDirectory("convertzz-audio-");
      const paths = {
        mp3: join(directory, "identify.mp3"),
        ape: join(directory, "identify.ape"),
        ogg: join(directory, "identify.ogg"),
        opus: join(directory, "identify.opus"),
      };
      generateAudio(paths.mp3, "libmp3lame");
      generateAudio(paths.ape, undefined, apeFixture);
      generateAudio(paths.ogg, "libvorbis");
      generateAudio(paths.opus, "libopus");
      const service = new AudioService(
        new ConversionService(resolve("ConvertZZ/Dictionary.csv")),
        wasmPath,
      );
      const scanned = await service.scan({ paths: Object.values(paths) });
      expect(new Set(scanned.map((file) => file.format))).toEqual(
        new Set(["mp3", "ape", "ogg", "opus"]),
      );
      expect(scanned.every((file) => !file.warning)).toBe(true);
    },
    30_000,
  );

  it.runIf(Boolean(ffmpegPath))(
    "多值欄位會逐值轉換",
    async () => {
      directory ||= await temporaryDirectory("convertzz-audio-");
      const path = join(directory, "multivalue.ogg");
      generateAudio(path, "libvorbis");
      taglib ??= await TagLib.initialize({ wasmUrl: wasmPath, forceWasmType: "wasi" });
      await taglib.edit(path, (audio) => {
        audio.setProperties({ artist: ["头发", "皇后"] });
      });
      const service = new AudioService(
        new ConversionService(resolve("ConvertZZ/Dictionary.csv")),
        wasmPath,
      );
      const [scan] = await service.scan({ paths: [path] });
      const artist = scan.fields.find((field) => field.key.toLowerCase() === "artist");
      expect(artist?.values).toEqual(["头发", "皇后"]);
      const plan = await service.plan({
        ...basePlanRequest([path]),
        selectedPaths: [path],
        selectedFields: { [path]: [`vorbis-comment:${artist!.key}`] },
      });
      expect(
        plan.files[0].fields.find((field) => field.key.toLowerCase() === "artist")?.convertedValues,
      ).toEqual(["頭髮", "皇后"]);
      await service.apply(plan.planId);
      const after = await readTagSnapshot(taglib, path);
      expect(property(after.properties, "artist")).toEqual(["頭髮", "皇后"]);
    },
    30_000,
  );
});

function audioService(): AudioService {
  return new AudioService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")), wasmPath);
}

function basePlanRequest(paths: string[]): AudioTagPlanRequest {
  return {
    paths,
    selectedPaths: paths,
    selectedFields: {},
    conversion: { direction: "s2t", engine: "segmented" },
    conflictPolicy: "skip",
    id3v1Enabled: true,
    id3v1Direction: "s2t",
    id3v1SourceEncoding: "gbk",
    id3v1OutputEncoding: "big5",
    id3v2Enabled: true,
    id3v2Direction: "s2t",
    id3v2Version: 4,
    id3v2Encoding: "utf8",
  };
}

function id3v1OnlyMp3(title: string, encoding: "big5" | "gbk"): Buffer {
  const audio = Buffer.from([0xff, 0xfb, 0x90, 0x64, 0, 0, 0, 0, 0, 0]);
  const tag = Buffer.alloc(128);
  tag.write("TAG", 0, "ascii");
  encodeText(title, encoding).copy(tag, 3, 0, 30);
  tag[127] = 255;
  return Buffer.concat([audio, tag]);
}

async function writeTaggedMp3(
  path: string,
  id3v1Title: string,
  id3v2Title: string,
  withPicture = false,
): Promise<void> {
  const audio = Buffer.from([0xff, 0xfb, 0x90, 0x64, 0, 0, 0, 0, 0, 0]);
  const parser = new MP3Tag(audio);
  parser.tags = {
    v2: {
      TIT2: id3v2Title,
      ...(withPicture
        ? { APIC: [{ format: "image/png", type: 3, description: "cover", data: [1, 2, 3, 4] }] }
        : {}),
    },
    v2Details: { version: [4, 0] },
  };
  const id3v2 = Buffer.from(
    parser.save({
      strict: false,
      id3v1: { include: false },
      id3v2: { include: true, version: 4, encoding: "utf8", unsupported: true },
    }) as ArrayBuffer,
  );
  if (parser.error) throw new Error(parser.error);
  const id3v1 = Buffer.alloc(128);
  id3v1.write("TAG", 0, "ascii");
  encodeText(id3v1Title, "gbk").copy(id3v1, 3, 0, 30);
  id3v1[127] = 255;
  await writeFile(path, Buffer.concat([id3v2, id3v1]));
}

function mp3AudioPayload(buffer: Buffer): Buffer {
  return Buffer.from(new MP3Tag(buffer).getAudio(true) as ArrayBuffer);
}

function id3v2HeaderVersion(buffer: Buffer): number {
  if (buffer.length < 4 || buffer.subarray(0, 3).toString("ascii") !== "ID3") {
    throw new Error("缺少 ID3v2 標頭");
  }
  return buffer[3];
}

function id3v2FrameTextEncoding(buffer: Buffer, frameId: string): number {
  const version = id3v2HeaderVersion(buffer);
  const tagSize =
    ((buffer[6] & 0x7f) << 21) |
    ((buffer[7] & 0x7f) << 14) |
    ((buffer[8] & 0x7f) << 7) |
    (buffer[9] & 0x7f);
  let offset = 10;
  const end = Math.min(buffer.length, 10 + tagSize);
  while (offset + 11 <= end) {
    const id = buffer.subarray(offset, offset + 4).toString("ascii");
    if (id === "\0\0\0\0") break;
    const frameSize =
      version === 4
        ? ((buffer[offset + 4] & 0x7f) << 21) |
          ((buffer[offset + 5] & 0x7f) << 14) |
          ((buffer[offset + 6] & 0x7f) << 7) |
          (buffer[offset + 7] & 0x7f)
        : buffer.readUInt32BE(offset + 4);
    if (id === frameId && frameSize > 0) return buffer[offset + 10];
    offset += 10 + frameSize;
  }
  throw new Error(`找不到 ${frameId} 文字編碼`);
}

function mp3PictureData(buffer: Buffer): unknown {
  const parser = new MP3Tag(buffer);
  const tags = parser.read({ id3v1: true, id3v2: true, unsupported: true });
  if (parser.error) throw new Error(parser.error);
  return tags.v2?.APIC;
}

function commandWorks(command: string, args: string[]): boolean {
  try {
    execFileSync(command, args, { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function resolveFfmpeg(): string | undefined {
  if (process.env.FFMPEG_BIN && existsSync(process.env.FFMPEG_BIN)) return process.env.FFMPEG_BIN;
  try {
    const bundled = require("ffmpeg-static") as string | null;
    if (bundled && existsSync(bundled)) return bundled;
  } catch {
    // 開發相依尚未安裝時改用系統 ffmpeg。
  }
  if (commandWorks("ffmpeg", ["-version"])) return "ffmpeg";
  return undefined;
}

function generateAudio(path: string, codec?: string, fixture?: string): void {
  if (fixture) {
    copyFileSync(fixture, path);
    return;
  }
  if (!codec || !ffmpegPath) throw new Error("音訊測試缺少編碼器或樣本。");
  execFileSync(ffmpegPath, [
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    "sine=frequency=440:duration=0.25",
    "-c:a",
    codec,
    "-y",
    path,
  ]);
}

async function temporaryDirectory(prefix: string): Promise<string> {
  const path = await mkdtemp(join(tmpdir(), prefix));
  temporaryDirectories.push(path);
  return path;
}

async function audioFingerprint(path: string, extension: string): Promise<string> {
  if (extension === "mp3") return mp3AudioPayload(await readFile(path)).toString("hex");
  return decodedAudioHash(path);
}

function decodedAudioHash(path: string): string {
  if (!ffmpegPath) throw new Error("音訊測試缺少 ffmpeg。");
  return execFileSync(
    ffmpegPath,
    ["-hide_banner", "-loglevel", "error", "-i", path, "-map", "0:a:0", "-f", "framemd5", "-"],
    { encoding: "utf8" },
  );
}

function coverPicture(): Picture {
  return {
    mimeType: "image/png",
    data: Uint8Array.from(
      Buffer.from(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl2nH0AAAAASUVORK5CYII=",
        "base64",
      ),
    ),
    type: "FrontCover",
    description: "ConvertZZ test cover",
  };
}

async function readTagSnapshot(
  taglib: TagLib,
  path: string,
): Promise<{ properties: PropertyMap; pictures: number[][] }> {
  const file = await taglib.open(path);
  try {
    return {
      properties: file.properties(),
      pictures: file.getPictures().map((picture) => Array.from(picture.data)),
    };
  } finally {
    file.dispose();
  }
}

function property(properties: PropertyMap, key: string): string[] | undefined {
  const match = Object.entries(properties).find(
    ([candidate]) => candidate.toLowerCase() === key.toLowerCase(),
  );
  return match?.[1];
}

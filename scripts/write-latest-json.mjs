import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function collectFiles(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) files.push(...collectFiles(path));
    else files.push(path);
  }
  return files;
}

function findSignedAsset(files, predicate) {
  const candidate = files.find((path) => {
    const name = basename(path);
    return predicate(name) && !name.endsWith(".sig");
  });
  if (!candidate) return null;
  const name = basename(candidate);
  const signaturePath = files.find((path) => basename(path) === `${name}.sig`);
  if (!signaturePath) throw new Error(`找不到 ${name} 的簽章檔。`);
  return {
    name,
    signature: readFileSync(signaturePath, "utf8").trim(),
  };
}

export function buildLatestJson({ files, tag, repo, pubDate }) {
  if (!tag) throw new Error("缺少發行標籤。");
  if (!repo) throw new Error("缺少 GitHub 倉庫名稱。");
  const version = tag.replace(/^v/iu, "");
  const downloadBase = `https://github.com/${repo}/releases/download/${tag}`;
  const windows = findSignedAsset(files, (name) => name.endsWith("-setup.exe"));
  const linux = findSignedAsset(files, (name) => name.endsWith(".AppImage"));
  /** @type {Record<string, { signature: string; url: string }>} */
  const platforms = {};
  if (windows) {
    platforms["windows-x86_64"] = {
      signature: windows.signature,
      url: `${downloadBase}/${windows.name}`,
    };
  }
  if (linux) {
    platforms["linux-x86_64"] = {
      signature: linux.signature,
      url: `${downloadBase}/${linux.name}`,
    };
  }
  if (Object.keys(platforms).length === 0) {
    throw new Error("找不到已簽署的 Windows NSIS 或 Linux AppImage。");
  }
  return {
    version,
    notes: `ConvertZZ ${tag}`,
    pub_date: pubDate,
    platforms,
  };
}

function readArg(name) {
  const index = process.argv.indexOf(`--${name}`);
  if (index === -1) return "";
  return process.argv[index + 1] ?? "";
}

const entry = process.argv[1];
if (entry && fileURLToPath(import.meta.url) === resolve(entry)) {
  const dir = readArg("dir");
  const tag = readArg("tag");
  const repo = readArg("repo");
  const out = readArg("out");
  if (!dir || !out) {
    process.stderr.write(
      "用法：node scripts/write-latest-json.mjs --dir <目錄> --tag <標籤> --repo <倉庫> --out <檔案>\n",
    );
    process.exit(1);
  }
  const payload = buildLatestJson({
    files: collectFiles(dir),
    tag,
    repo,
    pubDate: new Date().toISOString(),
  });
  writeFileSync(out, `${JSON.stringify(payload, null, 2)}\n`);
}

import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  createWriteStream,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { get } from "node:https";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";

export const QEMU_PASS = "CONVERTZZ_QEMU_PASS";
export const QEMU_FAIL = "CONVERTZZ_QEMU_FAIL";
export const QEMU_RESULT = "CONVERTZZ_QEMU_RESULT";
export const CLOUD_IMAGE_NAME = "jammy-server-cloudimg-amd64.img";
export const CLOUD_IMAGE_URL = `https://cloud-images.ubuntu.com/jammy/current/${CLOUD_IMAGE_NAME}`;
export const CLOUD_IMAGE_SUMS_URL = "https://cloud-images.ubuntu.com/jammy/current/SHA256SUMS";

export function cloudImageSources() {
  const custom = process.env.CONVERTZZ_QEMU_IMAGE_URL;
  const sources = [];
  if (custom)
    sources.push({
      name: "CONVERTZZ_QEMU_IMAGE_URL",
      image: custom,
      sums: process.env.CONVERTZZ_QEMU_IMAGE_SUMS_URL,
    });
  sources.push(
    {
      name: "twds",
      image: `https://mirror.twds.com.tw/ubuntu-cloud-images/jammy/current/${CLOUD_IMAGE_NAME}`,
      sums: "https://mirror.twds.com.tw/ubuntu-cloud-images/jammy/current/SHA256SUMS",
    },
    {
      name: "nchc",
      image: `https://free.nchc.org.tw/ubuntu-cloud-images/jammy/current/${CLOUD_IMAGE_NAME}`,
      sums: "https://free.nchc.org.tw/ubuntu-cloud-images/jammy/current/SHA256SUMS",
    },
    {
      name: "nchc-opensource",
      image: `https://opensource.nchc.org.tw/ubuntu-cloud-images/jammy/current/${CLOUD_IMAGE_NAME}`,
      sums: "https://opensource.nchc.org.tw/ubuntu-cloud-images/jammy/current/SHA256SUMS",
    },
    { name: "canonical", image: CLOUD_IMAGE_URL, sums: CLOUD_IMAGE_SUMS_URL },
  );
  return sources;
}

export function guestScript() {
  return [
    "#!/bin/bash",
    "set -euo pipefail",
    "exec > /dev/ttyS0 2>&1",
    "exec < /dev/null",
    `fail() { printf '%s %s\\n' "${QEMU_FAIL}" "$*"; sleep 2; poweroff -f || true; exit 1; }`,
    "for _ in $(seq 1 40); do",
    "  mkdir -p /mnt/share",
    "  if mountpoint -q /mnt/share || mount -t 9p -o trans=virtio,version=9p2000.L,ro hostshare /mnt/share; then",
    "    break",
    "  fi",
    "  sleep 2",
    "done",
    'test -f /mnt/share/guest.sh || fail "無法掛載主機分享目錄"',
    'command -v node >/dev/null && fail "乾淨映像不應已有 node"',
    'command -v npm >/dev/null && fail "乾淨映像不應已有 npm"',
    'dpkg -s nodejs >/dev/null 2>&1 && fail "乾淨映像不應已安裝 nodejs"',
    "export DEBIAN_FRONTEND=noninteractive",
    "apt-get update",
    "shopt -s nullglob",
    "debs=(/mnt/share/*.deb)",
    'test "${#debs[@]}" -eq 1 || fail "分享目錄應恰好有一個 DEB"',
    'apt-get install -y "${debs[0]}"',
    'command -v node >/dev/null && fail "安裝後不應出現 node"',
    'dpkg -s nodejs >/dev/null 2>&1 && fail "安裝後不應出現 nodejs"',
    'dpkg -s libayatana-appindicator3-dev >/dev/null 2>&1 && fail "不應安裝 AppIndicator 開發套件"',
    'dpkg -s convert-zz >/dev/null || fail "convert-zz 未安裝"',
    'dpkg -s libwebkit2gtk-4.1-0 >/dev/null || fail "APT 未補齊 WebKitGTK"',
    'dpkg -s libayatana-appindicator3-1 >/dev/null || fail "APT 未補齊 AppIndicator"',
    'dpkg -s libgtk-3-0 >/dev/null || fail "APT 未補齊 GTK"',
    'test -x /usr/bin/convertzz || fail "缺少主程式"',
    'test -f /usr/lib/ConvertZZ/Dictionary.csv || fail "缺少字典"',
    'test -d /usr/lib/ConvertZZ/segment-dict/segment || fail "缺少分詞字典"',
    'test ! -f /usr/lib/ConvertZZ/convertzz-sidecar.gz || fail "不應再包含 sidecar"',
    'test ! -f /usr/lib/ConvertZZ/taglib-wasi.wasm || fail "不應再包含 taglib WASM"',
    'WORKDIR="$(mktemp -d)"',
    "APPIMAGE=false",
    "appimages=(/mnt/share/*.AppImage)",
    'if test "${#appimages[@]}" -eq 1; then',
    '  EXTRACT="$WORKDIR/appimage"',
    '  mkdir -p "$EXTRACT"',
    '  cp "${appimages[0]}" "$EXTRACT/ConvertZZ.AppImage"',
    '  chmod +x "$EXTRACT/ConvertZZ.AppImage"',
    '  (cd "$EXTRACT" && ./ConvertZZ.AppImage --appimage-extract >/dev/null)',
    '  test -x "$EXTRACT/squashfs-root/usr/bin/convertzz" || fail "AppImage 缺少主程式"',
    '  test -d "$EXTRACT/squashfs-root/usr/lib/ConvertZZ/segment-dict/segment" || fail "AppImage 缺少分詞字典"',
    "  APPIMAGE=true",
    "fi",
    `printf '%s %s\\n' "${QEMU_RESULT}" "{\\"deb\\":true,\\"nodejs\\":false,\\"appindicatorDev\\":false,\\"webkit\\":true,\\"core\\":true,\\"appimageExtracted\\":$APPIMAGE}"`,
    `printf '%s\\n' "${QEMU_PASS}"`,
    "sleep 2",
    "poweroff -f || true",
    "",
  ].join("\n");
}

export function cloudInitUserData() {
  return `#cloud-config
datasource_list: [NoCloud]
package_update: false
package_upgrade: false
ssh_pwauth: false
bootcmd:
  - mkdir -p /mnt/share
  - mount -t 9p -o trans=virtio,version=9p2000.L,ro hostshare /mnt/share || true
runcmd:
  - [bash, /mnt/share/guest.sh]
`;
}

export function cloudInitMetaData() {
  return "instance-id: convertzz-qemu-1\nlocal-hostname: convertzz-qemu\n";
}

export function parseQemuSerial(serial) {
  const lines = serial.split(/\r?\n/u);
  const fail = lines.find((line) => line.startsWith(QEMU_FAIL));
  const pass = lines.some((line) => line.trim() === QEMU_PASS);
  const resultLine = lines.find((line) => line.startsWith(`${QEMU_RESULT} `));
  return {
    pass,
    fail,
    result: resultLine ? JSON.parse(resultLine.slice(QEMU_RESULT.length + 1)) : undefined,
  };
}

export function findLinuxArtifacts(projectRoot) {
  const root = resolve(projectRoot);
  const debDirectory = join(root, "src-tauri/target/release/bundle/deb");
  const appImageDirectory = join(root, "src-tauri/target/release/bundle/appimage");
  const deb = existsSync(debDirectory)
    ? readdirSync(debDirectory)
        .filter((name) => name.endsWith(".deb"))
        .map((name) => join(debDirectory, name))
        .sort()
    : [];
  const appImage = existsSync(appImageDirectory)
    ? readdirSync(appImageDirectory)
        .filter((name) => name.endsWith(".AppImage"))
        .map((name) => join(appImageDirectory, name))
        .sort()
    : [];
  return {
    deb: deb[0],
    appImage: appImage[0],
    ape: join(root, "tests/fixtures/mac-399.ape"),
    ogg: join(root, "tests/fixtures/test.ogg"),
  };
}

/** Host-side DEB sanity check before starting QEMU. */
export function assertLinuxDebReady(debPath) {
  const listing = listDebContents(debPath);
  const required = ["usr/lib/ConvertZZ/Dictionary.csv", "usr/lib/ConvertZZ/segment-dict/segment/"];
  const forbidden = [
    "usr/lib/ConvertZZ/convertzz-sidecar.gz",
    "usr/lib/ConvertZZ/taglib-wasi.wasm",
  ];
  const missing = required.filter((entry) => !listing.some((line) => line.includes(entry)));
  const presentForbidden = forbidden.filter((entry) =>
    listing.some((line) => line.includes(entry)),
  );
  if (missing.length === 0 && presentForbidden.length === 0) return;

  const hints = [
    `DEB 內容不符合目前核心發行契約：${debPath}`,
    missing.length ? `缺少：${missing.join("、")}` : "",
    presentForbidden.length ? `不應再包含：${presentForbidden.join("、")}` : "",
    "請先重建：pnpm tauri build --bundles deb,appimage",
  ].filter(Boolean);
  throw new Error(hints.join("\n"));
}

function listDebContents(debPath) {
  if (!existsSync(debPath)) throw new Error(`找不到 DEB：${debPath}`);
  const dpkg = spawnSync("dpkg-deb", ["-c", debPath], {
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (dpkg.status === 0) return dpkg.stdout.split(/\r?\n/u).filter(Boolean);

  // Fallback when dpkg-deb is unavailable: list data.tar.* members.
  const members = spawnSync("ar", ["t", debPath], { encoding: "utf8" });
  if (members.status !== 0) {
    throw new Error(`無法讀取 DEB 內容（需要 dpkg-deb 或 ar）：${debPath}`);
  }
  const dataMember = members.stdout.split(/\r?\n/u).find((name) => name.startsWith("data.tar"));
  if (!dataMember) throw new Error(`DEB 缺少 data.tar：${debPath}`);
  const extracted = spawnSync("ar", ["p", debPath, dataMember], {
    encoding: "buffer",
    maxBuffer: 256 * 1024 * 1024,
  });
  if (extracted.status !== 0) throw new Error(`無法抽出 ${dataMember}：${debPath}`);
  const tarArgs = dataMember.endsWith(".xz")
    ? ["-tJ"]
    : dataMember.endsWith(".gz")
      ? ["-tz"]
      : dataMember.endsWith(".zst")
        ? ["-t", "--zstd"]
        : ["-t"];
  const listed = spawnSync("tar", tarArgs, {
    encoding: "utf8",
    input: extracted.stdout,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (listed.status !== 0) throw new Error(`無法列出 ${dataMember}：${listed.stderr || debPath}`);
  return listed.stdout.split(/\r?\n/u).filter(Boolean);
}

export function qemuAvailable() {
  return Boolean(commandPath("qemu-system-x86_64") && commandPath("qemu-img") && isoTool());
}

function commandPath(command) {
  const result = spawnSync("/bin/sh", ["-c", `command -v ${command}`], { encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "";
}

function isoTool() {
  return commandPath("genisoimage") || commandPath("mkisofs") || commandPath("xorriso");
}

export function resolveLocalCloudImage(cacheDirectory) {
  const fromEnv = process.env.CONVERTZZ_QEMU_IMAGE;
  if (fromEnv) {
    const resolved = resolve(fromEnv);
    if (!existsSync(resolved)) throw new Error(`CONVERTZZ_QEMU_IMAGE 不存在：${resolved}`);
    return resolved;
  }
  const cached = join(cacheDirectory, CLOUD_IMAGE_NAME);
  return existsSync(cached) ? cached : undefined;
}

export async function downloadCloudImage(cacheDirectory) {
  mkdirSync(cacheDirectory, { recursive: true });
  const local = resolveLocalCloudImage(cacheDirectory);
  if (local) return local;

  const imagePath = join(cacheDirectory, CLOUD_IMAGE_NAME);
  const temporary = `${imagePath}.part`;
  const errors = [];
  for (const source of cloudImageSources()) {
    try {
      process.stderr.write(`下載 Ubuntu cloud image（${source.name}）…\n`);
      let expected;
      if (source.sums) {
        const sums = await downloadText(source.sums);
        const line = sums.split(/\r?\n/u).find((entry) => entry.endsWith(CLOUD_IMAGE_NAME));
        if (!line) throw new Error("找不到 SHA256SUMS 中的映像檔名。");
        expected = line.split(/\s+/u)[0].toLowerCase();
      }
      await downloadFile(source.image, temporary);
      if (expected) {
        const actual = sha256File(temporary);
        if (actual !== expected) throw new Error(`雜湊不符。預期 ${expected}，實際 ${actual}。`);
        writeFileSync(`${imagePath}.sha256`, `${expected}\n`);
      }
      copyFileSync(temporary, imagePath);
      rmSync(temporary, { force: true });
      return imagePath;
    } catch (error) {
      rmSync(temporary, { force: true });
      errors.push(`${source.name}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
  throw new Error(
    [
      "無法下載 Ubuntu 22.04 cloud image。",
      ...errors.map((entry) => `- ${entry}`),
      "可改把映像放到 tests/.cache/qemu/jammy-server-cloudimg-amd64.img，",
      "或設定 CONVERTZZ_QEMU_IMAGE=/path/to/jammy-server-cloudimg-amd64.img。",
    ].join("\n"),
  );
}

export async function runLinuxQemuVerification(options = {}) {
  const projectRoot = resolve(options.projectRoot ?? process.cwd());
  if (!qemuAvailable())
    throw new Error("本機沒有 qemu-system-x86_64、qemu-img 與 genisoimage／xorriso。");
  const artifacts = findLinuxArtifacts(projectRoot);
  if (!artifacts.deb) throw new Error("找不到 DEB。請先執行 pnpm tauri build --bundles deb。");
  assertLinuxDebReady(artifacts.deb);
  if (!existsSync(artifacts.ape) || !existsSync(artifacts.ogg))
    throw new Error("找不到 tests/fixtures 音訊樣本。");

  const cacheDirectory = join(projectRoot, "tests/.cache/qemu");
  const imagePath = await downloadCloudImage(cacheDirectory);
  const workDirectory = mkdtempSync(join(tmpdir(), "convertzz-qemu-"));
  const shareDirectory = join(workDirectory, "share");
  mkdirSync(shareDirectory);
  copyFileSync(artifacts.deb, join(shareDirectory, basename(artifacts.deb)));
  if (artifacts.appImage) {
    const sharedAppImage = join(shareDirectory, basename(artifacts.appImage));
    copyFileSync(artifacts.appImage, sharedAppImage);
    chmodSync(sharedAppImage, 0o755);
  }
  copyFileSync(artifacts.ape, join(shareDirectory, "mac-399.ape"));
  copyFileSync(artifacts.ogg, join(shareDirectory, "test.ogg"));
  writeFileSync(join(shareDirectory, "guest.sh"), guestScript());
  chmodSync(join(shareDirectory, "guest.sh"), 0o755);

  writeFileSync(join(workDirectory, "user-data"), cloudInitUserData());
  writeFileSync(join(workDirectory, "meta-data"), cloudInitMetaData());
  const seed = join(workDirectory, "seed.iso");
  createSeedIso(workDirectory, seed);
  const overlay = join(workDirectory, "disk.qcow2");
  runCommand("qemu-img", ["create", "-f", "qcow2", "-F", "qcow2", "-b", imagePath, overlay, "10G"]);

  const kvm = existsSync("/dev/kvm");
  const serialLog = join(workDirectory, "serial.log");
  const args = [
    "-accel",
    kvm ? "kvm" : "tcg",
    "-machine",
    "pc",
    "-m",
    "3072",
    "-smp",
    "2",
    "-display",
    "none",
    "-nographic",
    "-serial",
    `file:${serialLog}`,
    "-drive",
    `file=${overlay},if=virtio,format=qcow2`,
    "-drive",
    `file=${seed},if=virtio,media=cdrom,format=raw`,
    "-netdev",
    "user,id=net0",
    "-device",
    "virtio-net-pci,netdev=net0",
    "-virtfs",
    `local,path=${shareDirectory},mount_tag=hostshare,security_model=none,readonly=on`,
  ];

  const timeoutMs = options.timeoutMs ?? 20 * 60 * 1000;
  const qemu = spawnSync("qemu-system-x86_64", args, {
    encoding: "utf8",
    timeout: timeoutMs,
    maxBuffer: 16 * 1024 * 1024,
  });
  const serial = existsSync(serialLog)
    ? readFileSync(serialLog, "utf8")
    : `${qemu.stdout}\n${qemu.stderr}`;
  const parsed = parseQemuSerial(serial);
  if (!options.keepWorkDirectory) {
    rmSync(workDirectory, { recursive: true, force: true });
  }
  if (!parsed.pass) {
    throw new Error(
      parsed.fail || `QEMU 驗收失敗。結束碼 ${qemu.status}。\n${serial.slice(-4000)}`,
    );
  }
  return { ...parsed.result, kvm };
}

function createSeedIso(directory, destination) {
  const tool = isoTool();
  if (basename(tool) === "xorriso") {
    runCommand(
      tool,
      [
        "-as",
        "mkisofs",
        "-output",
        destination,
        "-volid",
        "cidata",
        "-joliet",
        "-rock",
        "user-data",
        "meta-data",
      ],
      directory,
    );
    return;
  }
  runCommand(
    tool,
    ["-output", destination, "-volid", "cidata", "-joliet", "-rock", "user-data", "meta-data"],
    directory,
  );
}

function runCommand(command, args, cwd) {
  const result = spawnSync(command, args, { encoding: "utf8", cwd });
  if (result.status !== 0) {
    throw new Error(`${basename(command)} 失敗：${result.stderr || result.stdout}`);
  }
}

function sha256File(path) {
  return spawnSync("sha256sum", [path], { encoding: "utf8" }).stdout.trim().split(/\s+/u)[0];
}

function downloadText(url) {
  if (commandPath("curl")) {
    const result = spawnSync(
      "curl",
      ["-fsSL", "--connect-timeout", "15", "--retry", "2", "--retry-all-errors", url],
      {
        encoding: "utf8",
        timeout: 60_000,
      },
    );
    if (result.status === 0) return Promise.resolve(result.stdout);
    return Promise.reject(new Error(result.stderr || `curl 結束碼 ${result.status}`));
  }
  return downloadBuffer(url).then((buffer) => buffer.toString("utf8"));
}

function downloadFile(url, destination) {
  if (commandPath("curl")) {
    const result = spawnSync(
      "curl",
      [
        "-fsSL",
        "--connect-timeout",
        "15",
        "--retry",
        "2",
        "--retry-all-errors",
        "-o",
        destination,
        url,
      ],
      {
        encoding: "utf8",
        timeout: 10 * 60 * 1000,
      },
    );
    if (result.status === 0 && existsSync(destination)) return Promise.resolve();
    rmSync(destination, { force: true });
    return Promise.reject(
      new Error(result.stderr || result.stdout || `curl 結束碼 ${result.status}`),
    );
  }
  return downloadBuffer(url).then((buffer) => {
    writeFileSync(destination, buffer);
  });
}

function downloadBuffer(url, redirects = 0) {
  if (redirects > 5) return Promise.reject(new Error(`重新導向次數過多：${url}`));
  return new Promise((resolvePromise, reject) => {
    const request = get(url, { family: 4, timeout: 15_000 }, (response) => {
      if (
        response.statusCode &&
        response.statusCode >= 300 &&
        response.statusCode < 400 &&
        response.headers.location
      ) {
        downloadBuffer(response.headers.location, redirects + 1).then(resolvePromise, reject);
        return;
      }
      if (response.statusCode !== 200) {
        reject(new Error(`下載失敗 ${response.statusCode}: ${url}`));
        return;
      }
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => resolvePromise(Buffer.concat(chunks)));
      response.on("error", reject);
    });
    request.on("timeout", () => {
      request.destroy(new Error(`連線逾時：${url}`));
    });
    request.on("error", reject);
  });
}

import { describe, expect, it } from "vitest";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  CLOUD_IMAGE_NAME,
  cloudImageSources,
  cloudInitUserData,
  findLinuxArtifacts,
  guestScript,
  parseQemuSerial,
  qemuAvailable,
  resolveLocalCloudImage,
  QEMU_FAIL,
  QEMU_PASS,
  QEMU_RESULT,
} from "../scripts/qemu-linux.mjs";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));

describe("QEMU Linux 乾淨環境驗收", () => {
  it("guest 腳本會安裝 DEB、拒絕 Node.js 與 -dev，並離線掃描音訊", () => {
    const script = guestScript();
    expect(script).toContain("apt-get install -y");
    expect(script).toContain("nodejs");
    expect(script).toContain("libayatana-appindicator3-dev");
    expect(script).toContain("libwebkit2gtk-4.1-0");
    expect(script).toContain("taglib-wasi.wasm");
    expect(script).toContain("unshare --net");
    expect(script).toContain("mac-399.ape");
    expect(script).toContain("--appimage-extract");
    expect(script).toContain('cp "${appimages[0]}" "$EXTRACT/ConvertZZ.AppImage"');
    expect(script).not.toContain('chmod +x "${appimages[0]}"');
    expect(script).toContain(QEMU_PASS);
    expect(cloudInitUserData()).toContain("/mnt/share/guest.sh");
  });

  it("解析序列埠通過與失敗輸出", () => {
    expect(
      parseQemuSerial(`boot\n${QEMU_RESULT} {"deb":true,"wasm":true}\n${QEMU_PASS}\n`),
    ).toMatchObject({
      pass: true,
      result: { deb: true, wasm: true },
    });
    expect(parseQemuSerial(`${QEMU_FAIL} 安裝後不應出現 node\n`)).toMatchObject({
      pass: false,
      fail: `${QEMU_FAIL} 安裝後不應出現 node`,
    });
  });

  it("尋找本機 Linux 發行檔與測試樣本", () => {
    const artifacts = findLinuxArtifacts(projectRoot);
    expect(existsSync(artifacts.ape)).toBe(true);
    expect(existsSync(artifacts.ogg)).toBe(true);
    expect(qemuAvailable()).toBeTypeOf("boolean");
  });

  it("映像下載會先試國內鏡像，並接受本機檔案", () => {
    const names = cloudImageSources().map((source) => source.name);
    expect(names).toEqual(expect.arrayContaining(["twds", "nchc", "nchc-opensource", "canonical"]));
    expect(cloudImageSources()[0]?.image).toContain("mirror.twds.com.tw");
    expect(resolveLocalCloudImage("/tmp/missing-qemu-cache")).toBeUndefined();
    expect(CLOUD_IMAGE_NAME).toContain("jammy");
  });
});

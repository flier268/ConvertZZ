import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

function readProjectFile(path: string): string {
  return readFileSync(fileURLToPath(new URL(`../${path}`, import.meta.url)), "utf8");
}

describe("發行工作流程契約", () => {
  const workflow = readProjectFile(".github/workflows/release.yml");
  const tauri = readProjectFile("src-tauri/tauri.conf.json");
  const updater = readProjectFile("src-tauri/tauri.updater.conf.json");

  it("J-04 Windows matrix 產出 NSIS 與 MSI", () => {
    expect(workflow).toContain("os: windows-latest");
    expect(workflow).toContain("artifact: windows-x64");
    expect(workflow).toContain("bundles: nsis,msi");
    expect(workflow).toContain("**/*.exe");
    expect(workflow).toContain("**/*.msi");
    expect(tauri).toContain('"targets": "all"');
    expect(tauri).toContain('"nsis"');
    expect(tauri).toContain('"wix"');
  });

  it("J-05 Linux matrix 產出 AppImage、DEB 與 RPM", () => {
    expect(workflow).toContain("os: ubuntu-22.04");
    expect(workflow).toContain("artifact: linux-x64");
    expect(workflow).toContain("bundles: appimage,deb,rpm");
    expect(workflow).toContain("**/*.AppImage");
    expect(workflow).toContain("**/*.deb");
    expect(workflow).toContain("**/*.rpm");
  });

  it("J-06 發行產物會建立 SHA-256 校驗檔", () => {
    expect(workflow).toContain("SHA256SUMS-linux-x64.txt");
    expect(workflow).toContain("SHA256SUMS-windows-x64.txt");
    expect(workflow).toContain("sha256sum src-tauri/target/release/bundle/appimage/*.AppImage");
    expect(workflow).toContain("Get-FileHash -Algorithm SHA256");
    expect(workflow).toContain("SHA256SUMS-*.txt");
  });

  it("J-12 會建立草稿 Release 並附上產物", () => {
    expect(workflow).toContain("draft-release:");
    expect(workflow).toContain("draft: true");
    expect(workflow).toContain("softprops/action-gh-release");
    expect(workflow).toContain("files: artifacts/**/*");
    expect(workflow).toContain("write-latest-json.mjs");
  });

  it("J-14 草稿說明會註明未提供作業系統程式碼簽章", () => {
    expect(workflow).toContain("此版本預設未提供作業系統程式碼簽章。");
    expect(workflow).toContain("DEB 與 RPM 請改從本頁下載。");
  });

  it("G-15 簽署更新只涵蓋 Windows 安裝程式與 Linux AppImage", () => {
    expect(updater).toContain('"createUpdaterArtifacts": true');
    expect(workflow).toContain("tauri.updater.conf.json");
    expect(workflow).toContain("TAURI_SIGNING_PRIVATE_KEY");
    expect(workflow).toContain("latest.json");
    expect(tauri).toContain(
      "https://github.com/flier268/ConvertZZ/releases/latest/download/latest.json",
    );
  });
});

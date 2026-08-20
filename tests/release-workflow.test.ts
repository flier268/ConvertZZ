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

  it("J-04 Windows matrix 產出 NSIS（預發行版略過 MSI）", () => {
    expect(workflow).toContain("os: windows-latest");
    expect(workflow).toContain("artifact: windows-x64");
    expect(workflow).toContain("bundles: nsis");
    expect(workflow).not.toContain("bundles: nsis,msi");
    expect(workflow).toContain("**/*.exe");
    expect(workflow).not.toContain("**/*.msi");
    expect(tauri).toContain('"nsis"');
    expect(tauri).toContain('"appimage"');
    expect(tauri).toContain('"deb"');
    expect(tauri).toContain('"rpm"');
    expect(tauri).not.toContain('"msi"');
    expect(tauri).toContain('"wix"');
  });

  it("Windows 發行會另外打包免安裝 zip", () => {
    expect(workflow).toContain("建立 Windows 免安裝版");
    expect(workflow).toContain("package-windows-portable.mjs");
    expect(workflow).toContain("src-tauri/target/release/bundle/portable");
    expect(workflow).toContain("**/*.zip");
    expect(workflow).toContain("$_.Extension -in '.exe', '.zip'");
    const packager = readProjectFile("scripts/package-windows-portable.mjs");
    expect(packager).toContain('PORTABLE_MARKER = "portable"');
    expect(packager).toContain("writeFileSync(join(stagedApp, PORTABLE_MARKER)");
    expect(packager).toContain("cwd: outDir");
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
    expect(workflow).toContain("name: ${{ env.RELEASE_TAG }}");
    expect(workflow).not.toContain("name: ConvertZZ ${{ env.RELEASE_TAG }}");
    expect(workflow).toContain("files: artifacts/**/*");
    expect(workflow).toContain("write-latest-json.mjs");
    expect(workflow).toContain("prerelease: ${{ contains(env.RELEASE_TAG, '-') }}");
  });

  it("草稿說明會註明各平台下載檔與自動更新範圍", () => {
    expect(workflow).toContain("## 下載說明");
    expect(workflow).toContain(
      "[`ConvertZZ_${{ env.RELEASE_VERSION }}_x64-setup.exe`](https://github.com/${{ github.repository }}/releases/download/${{ env.RELEASE_TAG }}/ConvertZZ_${{ env.RELEASE_VERSION }}_x64-setup.exe)",
    );
    expect(workflow).toContain(
      "[`ConvertZZ_${{ env.RELEASE_VERSION }}_x64-portable.zip`](https://github.com/${{ github.repository }}/releases/download/${{ env.RELEASE_TAG }}/ConvertZZ_${{ env.RELEASE_VERSION }}_x64-portable.zip)",
    );
    expect(workflow).toContain("settings-v2.json");
    expect(workflow).toContain("可整包帶走");
    expect(workflow).toContain(
      "[`ConvertZZ_${{ env.RELEASE_VERSION }}_amd64.AppImage`](https://github.com/${{ github.repository }}/releases/download/${{ env.RELEASE_TAG }}/ConvertZZ_${{ env.RELEASE_VERSION }}_amd64.AppImage)",
    );
    expect(workflow).toContain(
      "[`ConvertZZ_${{ env.RELEASE_VERSION }}_amd64.deb`](https://github.com/${{ github.repository }}/releases/download/${{ env.RELEASE_TAG }}/ConvertZZ_${{ env.RELEASE_VERSION }}_amd64.deb)",
    );
    expect(workflow).toContain(
      "[`ConvertZZ-${{ env.RELEASE_VERSION }}-1.x86_64.rpm`](https://github.com/${{ github.repository }}/releases/download/${{ env.RELEASE_TAG }}/ConvertZZ-${{ env.RELEASE_VERSION }}-1.x86_64.rpm)",
    );
    expect(workflow).toContain("RELEASE_VERSION=${RELEASE_TAG#v}");
    expect(workflow).toContain("Windows 安裝程式與 Linux AppImage 可用應用程式內自動更新。");
    expect(workflow).toContain("自動更新會驗證 latest.json 與安裝包簽章。");
    expect(workflow).toContain("DEB、RPM 與 Windows 免安裝 zip 請改從本頁下載。");
    expect(workflow).toContain("所有發行檔仍可用隨附的 SHA-256 檔案驗證。");
    expect(workflow).not.toContain("作業系統程式碼簽章");
  });

  it("發佈含 alpha／beta／rc 的標籤時會強制移動通道標籤", () => {
    const channelWorkflow = readProjectFile(".github/workflows/prerelease-channel.yml");
    expect(channelWorkflow).toContain("types: [published]");
    expect(channelWorkflow).toContain("prerelease-channel.mjs");
    expect(channelWorkflow).toContain("alpha／beta／rc");
    expect(channelWorkflow).toContain('git tag -f -a "$channel" -m "$VERSION_TAG"');
    expect(channelWorkflow).toContain('git push origin "refs/tags/$channel" --force');
    expect(channelWorkflow).not.toContain("per_page=30");
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

  it("Windows 發行會跑 pnpm check，且倉庫強制 LF 以免 Prettier 因 CRLF 失敗", () => {
    const gitattributes = readProjectFile(".gitattributes");
    const prettier = readProjectFile(".prettierrc.json");

    expect(workflow).toContain("pnpm run check");
    expect(workflow).toContain("os: windows-latest");
    expect(gitattributes).toMatch(/^\*\s+text=auto\s+eol=lf\s*$/m);
    expect(prettier).toContain('"endOfLine": "lf"');
  });
});

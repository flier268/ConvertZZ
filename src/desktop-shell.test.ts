import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

function readProjectFile(path: string): string {
  return readFileSync(fileURLToPath(new URL(`../${path}`, import.meta.url)), "utf8");
}

function readJson(path: string): Record<string, unknown> {
  return JSON.parse(readProjectFile(path)) as Record<string, unknown>;
}

describe("Tauri desktop shell", () => {
  it("grants explicit clipboard text permissions", () => {
    const main = readJson("src-tauri/capabilities/default.json");
    const floating = readJson("src-tauri/capabilities/floating.json");
    const required = ["clipboard-manager:allow-read-text", "clipboard-manager:allow-write-text"];

    expect(main.windows).toEqual(["main"]);
    expect(main.permissions).toEqual(expect.arrayContaining(required));
    expect(main.permissions).toEqual(expect.arrayContaining([
      "core:window:allow-hide",
      "core:window:allow-show",
      "global-shortcut:allow-register",
      "global-shortcut:allow-unregister-all",
    ]));
    expect(floating.windows).toEqual(["floating"]);
    expect(floating.permissions).toEqual(expect.arrayContaining(required));
    expect(floating.permissions).toEqual(expect.arrayContaining(["opener:default"]));
  });

  it("allows the floating window to move and persist its position", () => {
    const floating = readJson("src-tauri/capabilities/floating.json");
    const component = readProjectFile("src/FloatingBall.vue");

    expect(floating.permissions).toEqual(expect.arrayContaining([
      "core:window:allow-set-position",
      "core:window:allow-start-dragging",
      "core:window:allow-hide",
      "core:window:allow-show",
    ]));
    expect(component).toContain("onMoved");
    expect(component).toContain("saveSettings");
  });

  it("keeps the floating surface transparent and uses asymmetric letters", () => {
    const styles = readProjectFile("src/styles.css");
    const component = readProjectFile("src/FloatingBall.vue");
    const config = readJson("src-tauri/tauri.conf.json") as {
      app?: { windows?: Array<{ label?: string; transparent?: boolean }> };
    };

    expect(config.app?.windows?.find((window) => window.label === "floating")?.transparent).toBe(true);
    expect(styles).toContain("html.floating-window");
    expect(component).toContain("floating-z-large");
    expect(component).toContain("floating-z-small");
  });

  it("keeps the floating ball left and right click contract from the WPF design", () => {
    const component = readProjectFile("src/FloatingBall.vue");
    const rust = readProjectFile("src-tauri/src/lib.rs");

    expect(component).toContain("pointerIntent");
    expect(component).toContain("popupAppMenu");
    expect(component).toContain("FLOATING_CONTEXT_MENU");
    expect(component).not.toContain("dblclick");
    expect(component).not.toContain('run("s2t")');
    expect(component).not.toContain('run("t2s")');
    expect(rust).toContain("fn show_main_window");
    expect(rust).toContain("fn quit_app");
  });

  it("assigns an icon and left-click behavior to the tray", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    const config = readJson("src-tauri/tauri.conf.json") as { bundle?: { icon?: string[] } };

    expect(rust).toContain("default_window_icon");
    expect(rust).toContain("show_menu_on_left_click(false)");
    expect(rust).toContain("WindowEvent::CloseRequested");
    expect(rust).toContain('app.emit("app://legacy-action", id)');
    expect(rust).toContain(".text(\"a3\", \"Unicode 簡 → Unicode 繁\")");
    expect(rust).toContain(".text(\"b1\", \"文件/檔名轉換\")");
    expect(rust).toContain(".text(\"settings\", \"設定\")");
    for (const actionId of ["a1", "a4", "b2", "c3", "za1", "ze2", "1", "about", "report", "quit"]) {
      expect(rust).toContain(`.text("${actionId}"`);
    }
    expect(config.bundle?.icon).toEqual(expect.arrayContaining(["icons/icon.png", "icons/icon.ico"]));
    expect(config.bundle?.icon).not.toContain("../ConvertZZ/Windows Logo.png");
  });

  it("bundles the Linux sidecar as an untouched resource", () => {
    const linux = readJson("src-tauri/tauri.linux.conf.json") as {
      bundle?: { externalBin?: string[]; resources?: Record<string, string> };
    };
    const rust = readProjectFile("src-tauri/src/lib.rs");

    expect(linux.bundle?.externalBin).toEqual([]);
    expect(linux.bundle?.resources).toMatchObject({
      "binaries/convertzz-sidecar-linux-resource.gz": "convertzz-sidecar.gz",
      "binaries/convertzz-sidecar-linux-resource.sha256": "convertzz-sidecar.sha256",
    });
    expect(rust).toContain('resource_dir.join("convertzz-sidecar.gz")');
    expect(rust).toContain("prepare_linux_sidecar");
    expect(rust).toContain("PermissionsExt");
    expect(rust).toContain("GzDecoder");
    expect(rust).toContain("sha256_file(&destination)");
    expect(rust).toContain("permissions.set_mode(0o755)");
    const buildScript = readProjectFile("scripts/build-sidecar.mjs");
    expect(buildScript).toContain("convertzz-sidecar-linux-resource.gz");
    expect(buildScript).toContain("gzipSync");
  });

  it("verifies the real AppImage sidecar without network access", () => {
    const workflow = readProjectFile(".github/workflows/release.yml");
    const verifier = readProjectFile("scripts/verify-linux-appimage.mjs");

    expect(workflow).toContain("unshare --user --map-root-user --net");
    expect(workflow).toContain("node scripts/verify-linux-appimage.mjs");
    expect(verifier).toContain("AppImage 內的 sidecar 資源不應具有執行權限");
    expect(verifier).toContain("AppImage 解壓後的 sidecar 不符");
    expect(verifier).toContain('formats.join(",") !== "ape,ogg"');
    expect(verifier).toContain('operation: "convert.preview"');
  });
});

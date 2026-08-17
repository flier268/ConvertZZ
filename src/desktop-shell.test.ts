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
    expect(main.permissions).toEqual(
      expect.arrayContaining([
        "core:window:allow-hide",
        "core:window:allow-show",
        "core:window:allow-set-position",
        "global-shortcut:allow-register",
        "global-shortcut:allow-unregister-all",
        "process:allow-restart",
        "updater:default",
      ]),
    );
    expect(floating.windows).toEqual(["floating"]);
    expect(floating.permissions).toEqual(expect.arrayContaining(required));
    expect(floating.permissions).toEqual(expect.arrayContaining(["opener:default"]));
  });

  it("allows the floating window to move and persist its position", () => {
    const floating = readJson("src-tauri/capabilities/floating.json");
    const component = readProjectFile("src/FloatingBall.vue");

    expect(floating.permissions).toEqual(
      expect.arrayContaining([
        "core:window:allow-set-position",
        "core:window:allow-start-dragging",
        "core:window:allow-hide",
        "core:window:allow-show",
      ]),
    );
    expect(component).toContain("onMoved");
    expect(component).toContain("patchSavedSettings");
    expect(component).not.toContain("saveSettings");
  });

  it("keeps the floating surface transparent and uses asymmetric letters", () => {
    const styles = readProjectFile("src/styles.css");
    const component = readProjectFile("src/FloatingBall.vue");
    const config = readJson("src-tauri/tauri.conf.json") as {
      app?: {
        windows?: Array<{
          label?: string;
          transparent?: boolean;
          visible?: boolean;
          backgroundColor?: number[];
          shadow?: boolean;
          width?: number;
          height?: number;
        }>;
      };
    };
    expect(config.app?.windows?.find((window) => window.label === "main")?.visible).toBe(false);
    expect(config.app?.windows?.find((window) => window.label === "floating")?.visible).toBe(false);
    expect(config.app?.windows?.find((window) => window.label === "floating")?.transparent).toBe(
      true,
    );
    expect(
      config.app?.windows?.find((window) => window.label === "floating")?.backgroundColor,
    ).toEqual([0, 0, 0, 0]);
    expect(styles).toContain("html.floating-window");
    expect(styles).toContain("-webkit-user-select: none");
    expect(styles).not.toContain("drop-shadow");
    expect(config.app?.windows?.find((window) => window.label === "floating")?.width).toBe(72);
    expect(config.app?.windows?.find((window) => window.label === "floating")?.height).toBe(72);
    expect(config.app?.windows?.find((window) => window.label === "floating")?.shadow).toBe(false);
    expect(styles).toContain("width: 72px");
    expect(readProjectFile("src/BrandMark.vue")).toContain('width="72"');
    expect(readProjectFile("src-tauri/src/lib.rs")).toContain("LogicalSize::new(72.0, 72.0)");
    expect(component).toContain("BrandMark");
    expect(readProjectFile("src/BrandMark.vue")).toContain('class="brand-z"');
    expect(readProjectFile("src/BrandMark.vue")).toContain('class="brand-two"');
    expect(component).toContain("@selectstart.prevent");
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

  it("hides the main window on startup unless the setting is enabled", () => {
    const app = readProjectFile("src/App.vue");
    const settings = readProjectFile("src/pages/SettingsPage.vue");
    const desktop = readProjectFile("src/lib/desktop.ts");
    const rust = readProjectFile("src-tauri/src/lib.rs");

    expect(desktop).toContain("applyStartupWindowVisibility");
    expect(app).toContain("applyStartupWindowVisibility");
    expect(app).toContain("args.length > 0");
    expect(settings).toContain("showMainWindowOnStart");
    expect(settings).toContain("啟動時顯示主視窗");
    expect(rust).toContain("fn hide_startup_windows");
    expect(rust).toContain("fn create_configured_windows");
    expect(rust.indexOf("create_configured_windows(app)")).toBeLessThan(
      rust.indexOf("attach_sidecar(app)"),
    );
  });

  it("builds hidden windows before spawning the sidecar", () => {
    const config = readJson("src-tauri/tauri.conf.json") as {
      app?: { windows?: Array<{ label?: string; create?: boolean }> };
    };
    const rust = readProjectFile("src-tauri/src/lib.rs");

    expect(config.app?.windows?.every((window) => window.create === false)).toBe(true);
    expect(rust).toContain("fn create_configured_windows");
    expect(rust.indexOf("create_configured_windows(app)")).toBeLessThan(
      rust.indexOf("attach_sidecar(app)"),
    );
    expect(rust).toContain("spawn_blocking");
    expect(rust).toContain("轉換核心已終止");
    expect(rust).toContain("convertzz.log");
    expect(rust).toContain("fn app_log_path");
  });

  it("builds the Windows release binary without a console subsystem", () => {
    const main = readProjectFile("src-tauri/src/main.rs");

    expect(main).toContain("cfg_attr(not(debug_assertions)");
    expect(main).toContain('windows_subsystem = "windows"');
  });

  it("positions the floating ball before showing it", () => {
    const app = readProjectFile("src/App.vue");
    const ball = readProjectFile("src/FloatingBall.vue");
    const desktop = readProjectFile("src/lib/desktop.ts");
    const html = readProjectFile("index.html");

    expect(desktop).toContain("setPosition");
    expect(desktop).toContain("applyFloatingBallWindow");
    expect(readProjectFile("src-tauri/src/lib.rs")).toContain("set_background_color");
    expect(app).toContain("revealFloating: false");
    expect(ball).toContain("applyFloatingBallWindow");
    expect(html).toContain("floating-window");
  });

  it("configures signed in-app updates for the main window", () => {
    const config = readJson("src-tauri/tauri.conf.json") as {
      plugins?: { updater?: { pubkey?: string; endpoints?: string[] } };
    };
    const updaterConfig = readJson("src-tauri/tauri.updater.conf.json") as {
      bundle?: { createUpdaterArtifacts?: boolean };
    };
    const about = readProjectFile("src/pages/AboutPage.vue");
    const app = readProjectFile("src/App.vue");
    const rust = readProjectFile("src-tauri/src/lib.rs");

    expect(config.plugins?.updater?.pubkey).toContain(
      "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6",
    );
    expect(config.plugins?.updater?.endpoints).toEqual([
      "https://github.com/flier268/ConvertZZ/releases/latest/download/latest.json",
    ]);
    expect(updaterConfig.bundle?.createUpdaterArtifacts).toBe(true);
    expect(rust).toContain("tauri_plugin_updater::Builder");
    expect(rust).toContain("tauri_plugin_process::init");
    expect(app).toContain("promptForAppUpdate");
    expect(app).toContain("skippedVersion: settings.skippedUpdateVersion");
    expect(app).toContain("app_log_path");
    expect(app).toContain("記錄檔：");
    expect(about).toContain("promptForAppUpdate");
    expect(about).not.toContain("github.com/flier268/ConvertZZ/releases");
  });

  it("registers sidecar state before setup so invoke cannot miss manage()", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");

    expect(rust).toContain(".manage(SidecarState::starting())");
    expect(rust.indexOf(".manage(SidecarState::starting())")).toBeLessThan(rust.indexOf(".setup("));
    expect(rust).toContain("SidecarProcess::Starting");
    expect(rust).toContain("fn attach_sidecar");
    expect(rust).toContain("轉換核心無法啟動");
    expect(rust).not.toMatch(/let sidecar = start_sidecar[\s\S]*app\.manage\(sidecar\)/);
  });

  it("raises the production chunk size warning for Element Plus", () => {
    const vite = readProjectFile("vite.config.ts");

    expect(vite).toContain("chunkSizeWarningLimit: 1500");
  });

  it("runs frontend e2e against Vite with mocked Tauri APIs", () => {
    const vite = readProjectFile("vite.config.ts");
    const playwright = readProjectFile("e2e/playwright.config.ts");
    const mock = readProjectFile("e2e/mocks/tauri.ts");
    expect(vite).toContain("convertzz-e2e-tauri-mock");
    expect(vite).toContain("@tauri-apps/");
    expect(playwright).toContain("pnpm run dev:e2e");
    expect(playwright).toContain("cwd: root");
    expect(vite).toContain('host: process.env.CONVERTZZ_E2E === "1" ? "127.0.0.1"');
    expect(mock).toContain('case "sidecar_send"');
  });

  it("shows conversion prompts in a separate toast window", () => {
    const config = readJson("src-tauri/tauri.conf.json") as {
      app?: { windows?: Array<{ label?: string; visible?: boolean; decorations?: boolean }> };
    };
    const rust = readProjectFile("src-tauri/src/lib.rs");
    const actions = readProjectFile("src/lib/legacyActions.ts");
    const toast = readJson("src-tauri/capabilities/toast.json");

    expect(config.app?.windows?.find((window) => window.label === "toast")?.visible).toBe(false);
    expect(config.app?.windows?.find((window) => window.label === "toast")?.decorations).toBe(
      false,
    );
    expect(rust).toContain("fn show_toast");
    expect(rust).toContain("place_toast_near_cursor");
    expect(actions).toContain("showAppToast");
    expect(actions).not.toContain("ElMessage.success");
    expect(toast.windows).toEqual(["toast"]);
  });

  it("reuses the settings page instead of remounting it on every visit", () => {
    const app = readProjectFile("src/App.vue");
    const settings = readProjectFile("src/pages/SettingsPage.vue");

    expect(app).toContain('<keep-alive include="SettingsPage">');
    expect(settings).toContain('defineOptions({ name: "SettingsPage" })');
    expect(settings).toContain("getLoadedSettings");
    expect(settings).toContain('<el-tabs v-model="activeTab" class="settings-tabs">');
    expect(settings).toContain('name="general" lazy');
    expect(settings).toContain('name="hotkeys" lazy');
    expect(settings).toContain('name="floating" lazy');
    expect(settings).not.toContain("load_zhconvert_api_key");
  });

  it("keeps ConvertZZ.json import on settings page and first-run tour", () => {
    const settings = readProjectFile("src/pages/SettingsPage.vue");
    const tour = readProjectFile("src/OnboardingTour.vue");
    const loader = readProjectFile("src/lib/settings.ts");
    const about = readProjectFile("src/pages/AboutPage.vue");

    expect(settings).toContain("匯入 ConvertZZ.json");
    expect(settings).toContain("importLegacySettings");
    expect(settings).toContain("onSettingsReplaced");
    expect(settings).toContain("settings-save-bar");
    expect(settings).toContain("header-actions");
    expect(loader).toContain("notifySettingsReplaced");
    expect(loader).not.toContain("找到舊版 ConvertZZ.json");
    expect(tour).toContain("匯入舊版設定");
    expect(tour).toContain("importLegacySettings");
    expect(about).toContain("重看系統導覽");
  });

  it("builds the Windows MSI as a Traditional Chinese installer", () => {
    const config = readJson("src-tauri/tauri.conf.json") as {
      bundle?: {
        windows?: { wix?: { language?: Record<string, { localePath?: string }> } };
      };
    };
    const locale = readProjectFile("src-tauri/windows/wix/zh-TW.wxl");

    expect(config.bundle?.windows?.wix?.language?.["zh-TW"]?.localePath).toBe(
      "./windows/wix/zh-TW.wxl",
    );
    expect(locale).toContain('Culture="zh-TW"');
    expect(locale).toContain("啟動 ConvertZZ");
    expect(locale).not.toContain("en-US");
  });

  it("assigns an icon and left-click behavior to the tray", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    const config = readJson("src-tauri/tauri.conf.json") as { bundle?: { icon?: string[] } };

    expect(rust).toContain("default_window_icon");
    expect(rust).toContain("show_menu_on_left_click(false)");
    expect(rust).toContain("WindowEvent::CloseRequested");
    expect(rust).toContain('app.emit("app://legacy-action", id)');
    expect(rust).toContain('.text("a3", "Unicode 簡 → Unicode 繁")');
    expect(rust).toContain('.text("b1", "文件/檔名轉換")');
    expect(rust).toContain('.text("settings", "設定")');
    for (const actionId of ["a1", "a4", "b2", "c3", "za1", "ze2", "1", "about", "report", "quit"]) {
      expect(rust).toContain(`.text("${actionId}"`);
    }
    expect(config.bundle?.icon).toEqual(
      expect.arrayContaining(["icons/icon.png", "icons/icon.ico"]),
    );
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
    expect(rust).toContain("dictionary.to_string_lossy().into_owned()");
    expect(rust).toContain("wasm.to_string_lossy().into_owned()");
    expect(rust).toContain("prepare_linux_sidecar");
    expect(rust).toContain("PermissionsExt");
    expect(rust).toContain("GzDecoder");
    expect(rust).toContain("sha256_file(&destination)");
    expect(rust).toContain("permissions.set_mode(0o755)");
    const buildScript = readProjectFile("scripts/build-sidecar.mjs");
    expect(buildScript).toContain("convertzz-sidecar-linux-resource.gz");
    expect(buildScript).toContain("gzipSync");
  });

  it("follows Tauri sidecar + beforeDevCommand conventions", () => {
    const packageJson = readJson("package.json") as { scripts?: Record<string, string> };
    const config = readJson("src-tauri/tauri.conf.json") as {
      build?: { beforeDevCommand?: string; beforeBuildCommand?: string };
      bundle?: { externalBin?: string[] };
    };
    const buildScript = readProjectFile("scripts/build-sidecar.mjs");
    const ensureScript = readProjectFile("scripts/ensure-sidecar.mjs");

    // beforeDevCommand is only the frontend server (official config hook usage).
    expect(config.build?.beforeDevCommand).toBe("pnpm run dev:web");
    expect(config.build?.beforeDevCommand).not.toContain("sidecar");
    // Production still packages the sidecar before the frontend bundle step.
    expect(config.build?.beforeBuildCommand).toContain("sidecar:build");
    expect(config.bundle?.externalBin).toEqual(["binaries/convertzz-sidecar"]);
    // Official pattern: package sidecar first, then tauri dev.
    expect(packageJson.scripts?.dev).toBe("pnpm run sidecar:ensure && tauri dev");
    expect(packageJson.scripts?.["sidecar:ensure"]).toBe("node scripts/ensure-sidecar.mjs");
    expect(ensureScript).toContain("convertzz-sidecar-");
    expect(ensureScript).toContain("isSidecarStale");
    expect(ensureScript).toContain("Sidecar binary older than sources");
    expect(buildScript).toContain('--print", "host-tuple');
    expect(buildScript).toContain('resolve(root, "sidecar", ".build")');
    expect(buildScript).toContain("publishFile(stagingOutput, output)");
    expect(buildScript).toContain("convertzz-sidecar-${triple}${extension}");
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

  it("documents a QEMU clean Ubuntu guest for Linux package acceptance", () => {
    const qemu = readProjectFile("scripts/qemu-linux.mjs");
    const verifier = readProjectFile("scripts/verify-linux-qemu.mjs");
    const npm = readProjectFile("package.json");

    expect(npm).toContain("test:qemu");
    expect(verifier).toContain("runLinuxQemuVerification");
    expect(qemu).toContain("qemu-system-x86_64");
    expect(qemu).toContain("jammy-server-cloudimg-amd64.img");
    expect(qemu).toContain("mirror.twds.com.tw");
    expect(qemu).toContain("unshare --net");
    expect(qemu).toContain("libayatana-appindicator3-dev");
  });
});

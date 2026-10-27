import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { applyVersionSources } from "../scripts/bump-version.mjs";

function readProjectFile(path: string): string {
  return readFileSync(fileURLToPath(new URL(`../${path}`, import.meta.url)), "utf8");
}

describe("產品版本單一來源", () => {
  const packageJson = JSON.parse(readProjectFile("package.json")) as { version: string };
  const cargoToml = readProjectFile("src-tauri/Cargo.toml");
  const toolToml = readProjectFile("src-tauri/roundtrip-dict/Cargo.toml");
  const cargoLock = readProjectFile("src-tauri/Cargo.lock");
  const tauri = readProjectFile("src-tauri/tauri.conf.json");
  const mock = readProjectFile("e2e/mocks/tauri.ts");
  const completed = readProjectFile("docs/MIGRATION_COMPLETED.md");

  it("package.json 是唯一手改的產品版本", () => {
    expect(packageJson.version).toMatch(/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/);
    expect(cargoToml).toMatch(
      new RegExp(
        `\\[workspace\\.package\\]\\nversion = "${packageJson.version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"`,
      ),
    );
    expect(cargoToml).toContain("version.workspace = true");
    expect(toolToml).toContain("version.workspace = true");
    expect(toolToml).not.toMatch(/^version = "/m);
    expect(cargoLock).toContain(`name = "convertzz"\nversion = "${packageJson.version}"`);
    expect(cargoLock).toContain(`name = "roundtrip-dict"\nversion = "${packageJson.version}"`);
  });

  it("Tauri 與 e2e 模擬從 package.json 讀版本", () => {
    expect(JSON.parse(tauri).version).toBe("../package.json");
    expect(mock).toContain('import { version as appVersion } from "../../package.json"');
    expect(mock).not.toMatch(/currentVersion: "2\.0\.0-/);
    expect(mock).not.toMatch(/return "2\.0\.0-/);
  });

  it("VS Code 工作會呼叫 bump-version", () => {
    const tasks = JSON.parse(readProjectFile(".vscode/tasks.json")) as {
      tasks: Array<{ label?: string; command?: string }>;
      inputs: Array<{ id?: string; type?: string }>;
    };
    expect(
      tasks.tasks.some((task) => task.command === "pnpm bump-version ${input:appVersion}"),
    ).toBe(true);
    expect(
      tasks.inputs.some((input) => input.id === "appVersion" && input.type === "promptString"),
    ).toBe(true);
  });

  it("封存驗收不再寫死目前 beta 號", () => {
    expect(completed).toContain("pnpm bump-version");
    expect(completed).not.toMatch(/A-01 \| `package\.json` 的版本為 `2\.0\.0-/);
  });

  it("bump-version 只改 package.json 與 Cargo workspace／lock", () => {
    const next = applyVersionSources(
      {
        packageJson: readProjectFile("package.json"),
        cargoToml,
        cargoLock,
      },
      "2.0.0-beta99",
    );
    expect(JSON.parse(next.packageJson).version).toBe("2.0.0-beta99");
    expect(next.cargoToml).toContain('[workspace.package]\nversion = "2.0.0-beta99"');
    expect(next.cargoLock).toContain('name = "convertzz"\nversion = "2.0.0-beta99"');
    expect(next.cargoLock).toContain('name = "roundtrip-dict"\nversion = "2.0.0-beta99"');
    expect(() => applyVersionSources({ packageJson: "{}", cargoToml, cargoLock }, "nope")).toThrow(
      /版本號格式無效/,
    );
  });
});

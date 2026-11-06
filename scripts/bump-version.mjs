import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const VERSION_PATTERN = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

export function assertVersion(version) {
  if (!VERSION_PATTERN.test(version)) {
    throw new Error(`版本號格式無效：${version}`);
  }
  return version;
}

export function setPackageJsonVersion(source, version) {
  const parsed = JSON.parse(source);
  parsed.version = version;
  return `${JSON.stringify(parsed, null, 2)}\n`;
}

export function setWorkspacePackageVersion(toml, version) {
  const next = toml.replace(
    /(\[workspace\.package\]\r?\nversion\s*=\s*")[^"]+(")/,
    `$1${version}$2`,
  );
  if (next === toml) {
    throw new Error("找不到 [workspace.package] version");
  }
  return next;
}

export function setLockPackageVersion(lock, name, version) {
  const pattern = new RegExp(`(name = "${name}"\\nversion = ")[^"]+(")`);
  const next = lock.replace(pattern, `$1${version}$2`);
  if (next === lock) {
    throw new Error(`Cargo.lock 找不到套件 ${name}`);
  }
  return next;
}

export function applyVersionSources({ packageJson, cargoToml, cargoLock }, version) {
  const nextVersion = assertVersion(version);
  return {
    packageJson: setPackageJsonVersion(packageJson, nextVersion),
    cargoToml: setWorkspacePackageVersion(cargoToml, nextVersion),
    cargoLock: setLockPackageVersion(
      setLockPackageVersion(cargoLock, "convertzz", nextVersion),
      "roundtrip-dict",
      nextVersion,
    ),
  };
}

function readProject(path) {
  return readFileSync(join(ROOT, path), "utf8");
}

function writeProject(path, content) {
  writeFileSync(join(ROOT, path), content);
}

function main(argv) {
  const version = argv[0];
  if (!version) {
    throw new Error("用法：pnpm bump-version <版本號>");
  }
  const next = applyVersionSources(
    {
      packageJson: readProject("package.json"),
      cargoToml: readProject("src-tauri/Cargo.toml"),
      cargoLock: readProject("src-tauri/Cargo.lock"),
    },
    version,
  );
  writeProject("package.json", next.packageJson);
  writeProject("src-tauri/Cargo.toml", next.cargoToml);
  writeProject("src-tauri/Cargo.lock", next.cargoLock);
}

const isDirectRun = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isDirectRun) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}

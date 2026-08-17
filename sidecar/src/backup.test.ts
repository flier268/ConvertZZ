import { afterEach, describe, expect, it } from "vitest";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  createUserBackup,
  createUserBackups,
  pruneNestedBackupRoots,
  resolveBackupRoots,
  userBackupPath,
} from "./backup.js";

const temporary: string[] = [];
afterEach(async () =>
  Promise.all(temporary.splice(0).map((path) => rm(path, { recursive: true, force: true }))),
);

describe("使用者備份", () => {
  it("檔案備份路徑加上 .bak 後綴", () => {
    expect(userBackupPath("/tmp/note.txt")).toBe("/tmp/note.txt.bak");
    expect(userBackupPath("/tmp/docs")).toBe("/tmp/docs.bak");
  });

  it("剪除巢狀資料夾根，檔案若在資料夾內則不重複", () => {
    expect(
      pruneNestedBackupRoots([
        { path: "/data/docs", kind: "directory" },
        { path: "/data/docs/nested", kind: "directory" },
        { path: "/data/docs/a.txt", kind: "file" },
        { path: "/data/other.txt", kind: "file" },
      ]),
    ).toEqual([
      { path: "/data/docs", kind: "directory" },
      { path: "/data/other.txt", kind: "file" },
    ]);
  });

  it("resolveBackupRoots 對資料夾與檔案分類", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-backup-roots-"));
    temporary.push(directory);
    const folder = join(directory, "folder");
    const file = join(directory, "alone.txt");
    await mkdir(folder);
    await writeFile(file, "x");
    await writeFile(join(folder, "inside.txt"), "y");
    await expect(resolveBackupRoots([folder, file])).resolves.toEqual([
      { path: folder, kind: "directory" },
      { path: file, kind: "file" },
    ]);
  });

  it("createUserBackup 複製檔案與資料夾", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-backup-copy-"));
    temporary.push(directory);
    const file = join(directory, "a.txt");
    const folder = join(directory, "box");
    await writeFile(file, "file-content");
    await mkdir(folder);
    await writeFile(join(folder, "b.txt"), "folder-content");

    expect(await createUserBackup(file)).toBe(`${file}.bak`);
    expect(await readFile(`${file}.bak`, "utf8")).toBe("file-content");

    expect(await createUserBackup(folder)).toBe(`${folder}.bak`);
    expect(await readFile(join(`${folder}.bak`, "b.txt"), "utf8")).toBe("folder-content");
  });

  it("createUserBackups 只備份涵蓋受影響路徑的根", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-backup-select-"));
    temporary.push(directory);
    const kept = join(directory, "kept.txt");
    const skipped = join(directory, "skipped.txt");
    await writeFile(kept, "k");
    await writeFile(skipped, "s");
    const created = await createUserBackups(
      [
        { path: kept, kind: "file" },
        { path: skipped, kind: "file" },
      ],
      [kept],
    );
    expect(created).toEqual([`${kept}.bak`]);
    expect(await readdir(directory)).toEqual(expect.arrayContaining(["kept.txt", "kept.txt.bak"]));
    expect(await readdir(directory)).not.toContain("skipped.txt.bak");
  });
});

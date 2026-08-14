import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { runLinuxQemuVerification } from "./qemu-linux.mjs";

const result = await runLinuxQemuVerification({
  projectRoot: join(dirname(fileURLToPath(import.meta.url)), ".."),
});
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);


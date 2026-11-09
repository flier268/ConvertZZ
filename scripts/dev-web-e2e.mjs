import { spawn } from "node:child_process";

const child = spawn("pnpm", ["run", "dev:web"], {
  stdio: "inherit",
  env: { ...process.env, CONVERTZZ_E2E: "1" },
});
child.on("error", (error) => {
  console.error(error);
  process.exit(1);
});
child.on("exit", (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  process.exit(code ?? 1);
});

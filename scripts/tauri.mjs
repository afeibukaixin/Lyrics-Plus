import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const tauriCli = join(projectRoot, "node_modules", "@tauri-apps", "cli", "tauri.js");
const devRunner = join(projectRoot, "scripts", "cargo-dev-runner.sh");
const originalArgs = process.argv.slice(2);

const hasCustomRunner = originalArgs.includes("--runner") || originalArgs.includes("-r");
const args =
  process.platform === "darwin" && originalArgs[0] === "dev" && !hasCustomRunner
    ? ["dev", "--runner", devRunner, ...originalArgs.slice(1)]
    : originalArgs;

const child = spawn(process.execPath, [tauriCli, ...args], {
  cwd: projectRoot,
  env: process.env,
  stdio: "inherit",
});

child.on("error", (error) => {
  console.error(error);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});

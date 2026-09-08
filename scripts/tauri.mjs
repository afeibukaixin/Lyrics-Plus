import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const tauriCli = join(projectRoot, "node_modules", "@tauri-apps", "cli", "tauri.js");
const devRunner = join(projectRoot, "scripts", "cargo-dev-runner.sh");
const devConfig = join(projectRoot, "src-tauri", "tauri.dev.conf.json");
const DEFAULT_TELEMETRY_ENDPOINT =
  "https://stats.xiaoafei.cn/v1/events";
const originalArgs = process.argv.slice(2);

const isDevCommand = originalArgs[0] === "dev";
const isBuildCommand = originalArgs[0] === "build";
const hasCustomRunner = originalArgs.includes("--runner") || originalArgs.includes("-r");
let args =
  process.platform === "darwin" && isDevCommand && !hasCustomRunner
    ? ["dev", "--runner", devRunner, ...originalArgs.slice(1)]
    : originalArgs;

if (isDevCommand) {
  // Tauri 的配置参数必须位于第一个 `--` 前；将开发配置放在最后以覆盖其他 identifier 配置。
  const separatorIndex = args.indexOf("--");
  const insertIndex = separatorIndex === -1 ? args.length : separatorIndex;
  args = [
    ...args.slice(0, insertIndex),
    "--config",
    devConfig,
    ...args.slice(insertIndex),
  ];
}

const childEnv = { ...process.env };
if (isDevCommand) {
  // 开发模式不向生产统计数据集发送事件，即使父 shell 曾设置过该变量。
  delete childEnv.LYRICS_PLUS_TELEMETRY_ENDPOINT;
} else if (isBuildCommand) {
  // 分发构建默认使用生产 Worker；非空环境变量可覆盖它以支持其他部署环境。
  const configuredEndpoint = childEnv.LYRICS_PLUS_TELEMETRY_ENDPOINT?.trim();
  childEnv.LYRICS_PLUS_TELEMETRY_ENDPOINT =
    configuredEndpoint || DEFAULT_TELEMETRY_ENDPOINT;
}

const child = spawn(process.execPath, [tauriCli, ...args], {
  cwd: projectRoot,
  env: childEnv,
  stdio: "inherit",
});

child.on("error", (error) => {
  console.error("Failed to start the Tauri CLI process:", error);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});

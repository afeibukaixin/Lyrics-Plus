import { spawn } from "node:child_process";
import { chmod, copyFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const helperRoot = join(projectRoot, "src-tauri", "player-follower");
const arch = process.env.TAURI_ENV_ARCH ?? process.arch;
const target = arch === "aarch64" || arch === "arm64"
  ? "aarch64-apple-darwin"
  : arch === "x86_64" || arch === "x64"
    ? "x86_64-apple-darwin"
    : null;

if (!target) throw new Error(`Unsupported macOS architecture: ${arch}`);

await new Promise((resolve, reject) => {
  const child = spawn("cargo", [
    "build",
    "--locked",
    "--release",
    "--target",
    target,
    "--manifest-path",
    join(helperRoot, "Cargo.toml"),
  ], { cwd: projectRoot, stdio: "inherit" });
  child.on("error", reject);
  child.on("exit", (code) => code === 0 ? resolve() : reject(new Error(`Helper build failed with exit code ${code}`)));
});

const destinationDir = join(projectRoot, "src-tauri", "binaries");
const source = join(helperRoot, "target", target, "release", "lyrics-plus-player-follower");
const destination = join(destinationDir, `lyrics-plus-player-follower-${target}`);
await mkdir(destinationDir, { recursive: true });
await copyFile(source, destination);
await chmod(destination, 0o755);

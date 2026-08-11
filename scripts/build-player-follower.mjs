import { spawn } from "node:child_process";
import { chmod, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const helperRoot = join(projectRoot, "src-tauri", "player-follower");
const tauriRoot = join(projectRoot, "src-tauri");
const arch = process.env.TAURI_ENV_ARCH ?? process.arch;
const target = arch === "aarch64" || arch === "arm64"
  ? "aarch64-apple-darwin"
  : arch === "x86_64" || arch === "x64"
    ? "x86_64-apple-darwin"
    : null;

if (!target) throw new Error(`Unsupported macOS architecture: ${arch}`);

const run = (command, args) => new Promise((resolve, reject) => {
  const child = spawn(command, args, { cwd: projectRoot, stdio: "inherit" });
  child.on("error", reject);
  child.on("exit", (code) => code === 0 ? resolve() : reject(new Error(`${command} failed with exit code ${code}`)));
});

await run("cargo", [
    "build",
    "--locked",
    "--release",
    "--target",
    target,
    "--manifest-path",
    join(helperRoot, "Cargo.toml"),
]);

const destinationBundle = join(tauriRoot, "binaries", "Lyrics Plus Player Follower.app");
const destinationContents = join(destinationBundle, "Contents");
const destinationDir = join(destinationContents, "MacOS");
const source = join(helperRoot, "target", target, "release", "lyrics-plus-player-follower");
const destination = join(destinationDir, "lyrics-plus-player-follower");
await mkdir(destinationDir, { recursive: true });
await copyFile(source, destination);
await chmod(destination, 0o755);

const { version } = JSON.parse(await readFile(join(tauriRoot, "tauri.conf.json"), "utf8"));
const infoPlist = (await readFile(join(helperRoot, "Info.plist"), "utf8")).replaceAll("__VERSION__", version);
await writeFile(join(destinationContents, "Info.plist"), infoPlist);

const signingIdentity = process.env.APPLE_SIGNING_IDENTITY ?? "-";
await run("codesign", ["--force", "--options", "runtime", "--sign", signingIdentity, destinationBundle]);
await run("codesign", ["--verify", "--deep", "--strict", destinationBundle]);

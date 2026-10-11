// Remap development paths before compiling public Windows artifacts.
// This does not enable signing or change the runtime's filesystem permissions.
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const root = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
if (
  args.some(
    (arg) =>
      ["--debug", "-d", "--target", "--profile", "--runner"].includes(arg) ||
      /^--(target|profile|runner)=/.test(arg),
  )
)
  throw new Error(
    "This path-checked packager supports native optimized builds only. Other profiles and targets require their own artifact validation.",
  );
if (process.platform !== "win32")
  throw new Error(
    "ClutterSift desktop packaging currently supports Windows only.",
  );
if (process.env.RUSTFLAGS && !process.env.CARGO_ENCODED_RUSTFLAGS)
  throw new Error(
    "Use CARGO_ENCODED_RUSTFLAGS for custom compiler flags so path remapping can preserve them safely.",
  );
const home = os.homedir();
const flags = [
  process.env.CARGO_ENCODED_RUSTFLAGS,
  `--remap-path-prefix=${home}=C:\\ClutterSiftBuild`,
  `--remap-path-prefix=${root}=C:\\ClutterSiftSource`,
]
  .filter(Boolean)
  .join("\x1f");
const result = spawnSync(
  process.execPath,
  [
    path.join(root, "node_modules", "@tauri-apps", "cli", "tauri.js"),
    "build",
    ...args,
  ],
  {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, CARGO_ENCODED_RUSTFLAGS: flags },
  },
);
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
const executable = path.join(
  root,
  "src-tauri",
  "target",
  "release",
  "cluttersift.exe",
);
if (!fs.existsSync(executable))
  throw new Error(
    "Expected native release executable was not produced. Custom targets must supply their own artifact-path validation.",
  );
const { version } = JSON.parse(
  fs.readFileSync(path.join(root, "package.json"), "utf8"),
);
const installer = path.join(
  root,
  "src-tauri",
  "target",
  "release",
  "bundle",
  "nsis",
  `ClutterSift_${version}_x64-setup.exe`,
);
for (const artifact of args.includes("--no-bundle")
  ? [executable]
  : [executable, installer]) {
  if (!fs.existsSync(artifact))
    throw new Error("Expected release artifact was not produced.");
  const bytes = fs.readFileSync(artifact);
  for (const prefix of [
    home,
    root,
    home.replaceAll("\\", "/"),
    root.replaceAll("\\", "/"),
  ]) {
    if (
      bytes.includes(Buffer.from(prefix, "utf8")) ||
      bytes.includes(Buffer.from(prefix, "utf16le"))
    )
      throw new Error(
        "Release artifact contains a private local build path. Do not distribute it.",
      );
  }
}
console.log(
  "PASS: packaged artifacts contain no current home/workspace paths in UTF-8 or UTF-16.",
);

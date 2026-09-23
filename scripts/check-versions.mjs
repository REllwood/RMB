import { readFileSync } from "node:fs";

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

const packageJson = readJson("package.json");
const packageLock = readJson("package-lock.json");
const tauriConfig = readJson("src-tauri/tauri.conf.json");
const cargoManifest = readFileSync("Cargo.toml", "utf8");
const workspacePackage = cargoManifest.match(
  /\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
);

if (!workspacePackage) {
  throw new Error("Could not read the workspace package version from Cargo.toml");
}

const versions = {
  "Cargo workspace": workspacePackage[1],
  "npm package": packageJson.version,
  "npm lockfile": packageLock.packages?.[""]?.version,
  "Tauri bundle": tauriConfig.version,
};
const missing = Object.entries(versions).filter(([, version]) => typeof version !== "string");

if (missing.length > 0) {
  throw new Error(`Missing version metadata: ${missing.map(([name]) => name).join(", ")}`);
}

const unique = new Set(Object.values(versions));
if (unique.size !== 1) {
  const details = Object.entries(versions)
    .map(([name, version]) => `${name}=${version}`)
    .join(", ");
  throw new Error(`Release versions do not match: ${details}`);
}

console.log(`Release version metadata is aligned at ${packageJson.version}.`);

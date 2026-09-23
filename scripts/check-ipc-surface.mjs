import { readFileSync } from "node:fs";

const frontendSource = readFileSync("src/lib/ipc.ts", "utf8");
const backendSource = readFileSync("src-tauri/src/lib.rs", "utf8");

const frontendCommands = [
  ...frontendSource.matchAll(/\binvoke(?:<[^>]+>)?\(\s*"([a-z0-9_]+)"/g),
].map((match) => match[1]);

const handler = backendSource.match(/\.invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\]\)/);
if (!handler) {
  throw new Error("Could not find the Tauri command registration list");
}

const backendCommands = [
  ...handler[1].matchAll(/\bcommands(?:::[a-z0-9_]+)*::([a-z][a-z0-9_]*)/g),
].map((match) => match[1]);

function duplicates(values) {
  return [...new Set(values.filter((value, index) => values.indexOf(value) !== index))].sort();
}

const duplicateFrontend = duplicates(frontendCommands);
const duplicateBackend = duplicates(backendCommands);
if (duplicateFrontend.length > 0 || duplicateBackend.length > 0) {
  throw new Error(
    `Duplicate IPC commands found. Frontend: ${duplicateFrontend.join(", ") || "none"}; backend: ${duplicateBackend.join(", ") || "none"}`,
  );
}

const frontend = new Set(frontendCommands);
const backend = new Set(backendCommands);
const missingBackend = [...frontend].filter((command) => !backend.has(command)).sort();
const missingFrontend = [...backend].filter((command) => !frontend.has(command)).sort();

if (missingBackend.length > 0 || missingFrontend.length > 0) {
  throw new Error(
    [
      `Frontend commands missing from Tauri: ${missingBackend.join(", ") || "none"}`,
      `Registered commands missing from the typed client: ${missingFrontend.join(", ") || "none"}`,
    ].join("\n"),
  );
}

console.log(`${frontend.size} IPC commands are registered and exposed by the typed client.`);

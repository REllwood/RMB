import { invoke } from "@tauri-apps/api/core";

/**
 * Typed client for Tauri commands — the single boundary between the React UI and the Rust
 * backend. Every command call goes through here so signatures live in one place and the UI
 * never touches `invoke` directly. Grows one method per command as features land.
 */
export const ipc = {
  greet: (name: string) => invoke<string>("greet", { name }),
};

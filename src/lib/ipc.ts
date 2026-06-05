import { invoke } from "@tauri-apps/api/core";

/**
 * Typed client for Tauri commands — the single boundary between the React UI and the Rust
 * backend. Every command call goes through here so signatures live in one place and the UI
 * never touches `invoke` directly. Grows one method per command as features land.
 */
export const ipc = {
  greet: (name: string) => invoke<string>("greet", { name }),

  /** App-meta key/value store (schema version, UI preferences like theme). */
  getMeta: (key: string) => invoke<string | null>("get_meta", { key }),
  setMeta: (key: string, value: string) => invoke<void>("set_meta", { key, value }),

  /** Database backup/restore. Paths are chosen via the dialog plugin in the UI. */
  backupDatabase: (dest: string) => invoke<void>("backup_database", { dest }),
  restoreDatabase: (src: string) => invoke<void>("restore_database", { src }),
};

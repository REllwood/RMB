import "@testing-library/jest-dom/vitest";

// Tauri's `invoke` is not available in jsdom; tests mock `@/lib/ipc` per-file as needed.

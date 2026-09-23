import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// Component/unit tests run in jsdom. Tailwind is intentionally not loaded here — axe's
// colour-contrast rule cannot run in jsdom, so design-token contrast has a deterministic test,
// which reads index.css as plain text (the only stylesheet tests load).
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./test/setup.ts"],
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    css: { include: [/index\.css/] },
  },
});

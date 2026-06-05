import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// Component/unit tests run in jsdom. Tailwind is intentionally not loaded here — axe's
// color-contrast rule can't run in jsdom anyway (it's checked by the Playwright pass).
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./test/setup.ts"],
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    css: false,
  },
});

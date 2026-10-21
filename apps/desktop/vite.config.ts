import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  server: { port: 1420, strictPort: true },
  clearScreen: false,
  test: {
    environment: "jsdom",
    setupFiles: ["./src/tests/setup.ts"],
    restoreMocks: true,
  },
  build: { target: "es2022" },
});

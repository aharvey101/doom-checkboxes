import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 60000,
  use: { baseURL: "http://127.0.0.1:8090" },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});

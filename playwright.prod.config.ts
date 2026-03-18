import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 60000,
  use: { baseURL: "https://rust-frontend-production.up.railway.app" },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});

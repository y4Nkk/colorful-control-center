import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.mjs",
  workers: 1,
  timeout: 60_000,
  outputDir: "../../target/ui-check/results",
  reporter: "list",
  use: { baseURL: "http://127.0.0.1:1450", browserName: "chromium", channel: "msedge", viewport: { width: 1000, height: 720 },
    reducedMotion: "reduce", launchOptions: { args: ["--disable-gpu"] }, trace: "retain-on-failure" },
  webServer: { command: "node server.mjs", url: "http://127.0.0.1:1450", timeout: 60_000, reuseExistingServer: false },
});

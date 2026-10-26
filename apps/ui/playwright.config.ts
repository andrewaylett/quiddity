import { defineConfig, devices } from "@playwright/test";

/**
 * Browser tests, run against the static export (`pnpm ui build` first). The UI
 * works on a made-up graph in memory outside the app, so each test starts from
 * the same graph and nothing is shared between them.
 */
export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? [["list"], ["github"]] : "list",
  use: { baseURL: "http://localhost:4173", trace: "retain-on-failure" },
  webServer: {
    command: "pnpm exec serve out --listen 4173 --no-clipboard",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});

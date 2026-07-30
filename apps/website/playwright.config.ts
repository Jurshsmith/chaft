import { defineConfig } from "@playwright/test";

const localBaseUrl = "http://127.0.0.1:4325";
const externalBaseUrl = process.env.PLAYWRIGHT_BASE_URL;

export default defineConfig({
  testDir: "./tests/previews",
  testMatch: "**/*.pw.ts",
  outputDir: "./test-results/previews",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  ...(process.env.CI ? { workers: 2 } : {}),
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  reporter: process.env.CI
    ? [["line"], ["html", { open: "never", outputFolder: "playwright-report" }]]
    : [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
  use: {
    baseURL: externalBaseUrl ?? localBaseUrl,
    colorScheme: "light",
    locale: "en-US",
    timezoneId: "UTC",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "off",
  },
  ...(externalBaseUrl
    ? {}
    : {
        webServer: {
          command:
            "pnpm exec astro preview --host 127.0.0.1 --port 4325",
          url: localBaseUrl,
          reuseExistingServer: !process.env.CI,
          timeout: 60_000,
        },
      }),
  projects: [
    {
      name: "chromium-320",
      use: {
        browserName: "chromium",
        viewport: { width: 320, height: 800 },
        hasTouch: true,
        isMobile: true,
      },
    },
    {
      name: "chromium-390",
      use: {
        browserName: "chromium",
        viewport: { width: 390, height: 844 },
        hasTouch: true,
        isMobile: true,
      },
    },
    {
      name: "chromium-768",
      use: {
        browserName: "chromium",
        viewport: { width: 768, height: 1024 },
        hasTouch: true,
      },
    },
    {
      name: "chromium-1440",
      use: {
        browserName: "chromium",
        viewport: { width: 1440, height: 900 },
      },
    },
    {
      name: "firefox-1440",
      use: {
        browserName: "firefox",
        viewport: { width: 1440, height: 900 },
      },
    },
    {
      name: "webkit-1440",
      use: {
        browserName: "webkit",
        viewport: { width: 1440, height: 900 },
      },
    },
  ],
});

const { defineConfig } = require('@playwright/test');
module.exports = defineConfig({
  testDir: '.',
  testMatch: 'playground.spec.cjs',
  fullyParallel: true,
  workers: process.env.CI ? 2 : undefined,
  forbidOnly: !!process.env.CI,
  retries: 0,
  use: {
    baseURL: process.env.PXR_TEST_URL || 'http://127.0.0.1:8080',
    launchOptions: process.env.PXR_BROWSER_PATH ? { executablePath: process.env.PXR_BROWSER_PATH } : {},
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'desktop', use: { viewport: { width: 1440, height: 1000 } } },
    { name: 'mobile', use: { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true } },
  ],
  webServer: process.env.PXR_TEST_URL ? undefined : {
    command: 'python -m http.server 8080 --bind 127.0.0.1 --directory ../../dist/playground',
    url: 'http://127.0.0.1:8080',
    reuseExistingServer: false,
  },
});

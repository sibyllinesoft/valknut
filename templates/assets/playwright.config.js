/**
 * Playwright Configuration for React Error #31 Debugging
 */

module.exports = {
  testDir: './tests/playwright',
  timeout: 30000,
  retries: 0, // No retries for debugging - we want to see failures immediately
  use: {
    headless: false, // Run in headed mode to see what's happening
    viewport: { width: 1280, height: 720 },
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { 
        ...require('@playwright/test').devices['Desktop Chrome'],
        // Enable detailed console logging
        launchOptions: {
          args: [
            '--enable-logging=stderr',
            '--log-level=0',
            '--disable-web-security',
            '--allow-running-insecure-content'
          ]
        }
      },
    },
  ],
  reporter: [
    ['list'],
    ['html', { open: 'never', outputFolder: 'playwright-report' }]
  ],
  outputDir: 'test-results',
};
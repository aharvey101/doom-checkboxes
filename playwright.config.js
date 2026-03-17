import { defineConfig } from '@playwright/test';
import path from 'path';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);

export default defineConfig({
  // Run tests from project root directory
  testDir: '.',
  testMatch: [
    '*.spec.js'                     // JavaScript integration tests in root directory
  ],
  
  // SpacetimeDB and collaborative features need time
  timeout: 30000, // Increase individual test timeout to reduce retries
  expect: { timeout: 10000 }, // Increase expect timeout for SpacetimeDB operations
  
  // Handle SpacetimeDB startup delays and occasional connection issues
  retries: process.env.CI ? 0 : 0, // Disable retries to save time - rely on proper test design
  workers: 1, // Sequential execution for collaborative state management
  
  // Global configuration
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000',
    trace: 'on-first-retry',
    video: 'retain-on-failure',
    screenshot: 'only-on-failure',
    // Additional browser settings for SpacetimeDB WebSocket testing
    permissions: ['clipboard-read', 'clipboard-write'],
    viewport: { width: 1280, height: 720 },
  },
  
  // Environment-specific projects
  projects: [
    {
      name: 'ci',
      use: {
        baseURL: 'http://localhost:5173',
      },
    },
    {
      name: 'staging', 
      use: {
        baseURL: process.env.STAGING_URL || 'https://checkbox-grid-staging.netlify.app',
      },
    },
    {
      name: 'production',
      use: {
        baseURL: process.env.PRODUCTION_URL || 'https://checkbox-grid-100x100.netlify.app',
      },
    }
  ],
  
  // Auto-start dev server for CI environment
  webServer: process.env.TEST_ENV === 'ci' ? {
    command: 'cd typescript-frontend && npm run dev -- --host 0.0.0.0 --port 5173',
    port: 5173,
    reuseExistingServer: !process.env.CI,
    timeout: 60000, // Reduce timeout to 1 minute
    stdout: 'pipe',
    stderr: 'pipe',
  } : undefined,
  
  // Global setup/teardown for SpacetimeDB state management
  globalSetup: require.resolve('./test-setup.js'),
  globalTeardown: require.resolve('./test-teardown.js'),
  
  // Reporting
  reporter: [
    ['html'],
    ['json', { outputFile: 'playwright-report/results.json' }],
    process.env.CI ? ['github'] : ['list']
  ],
  
  // Output directories
  outputDir: 'test-results/',
});
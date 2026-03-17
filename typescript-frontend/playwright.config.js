import { defineConfig } from '@playwright/test';
import path from 'path';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);

export default defineConfig({
  // Run both root-level integration tests and local E2E tests
  testDir: '../',
  testMatch: ['*.spec.js', 'typescript-frontend/tests/**/*.spec.js'],
  
  // SpacetimeDB and collaborative features need time
  timeout: 30000,
  expect: { timeout: 10000 },
  
  // Handle SpacetimeDB startup delays and occasional connection issues
  retries: process.env.CI ? 2 : 1,
  workers: 1, // Sequential execution for collaborative state management
  
  // Global configuration
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000',
    trace: 'on-first-retry',
    video: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  
  // Environment-specific projects
  projects: [
    {
      name: 'ci',
      use: {
        baseURL: 'http://localhost:8000',
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
    command: 'npm run dev',
    port: 8000,
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
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
  
  // Browser configuration
  use: {
    ...((typeof module !== 'undefined' && module.exports || {}).use || {}),
    // Additional browser settings for SpacetimeDB WebSocket testing
    permissions: ['clipboard-read', 'clipboard-write'],
    viewport: { width: 1280, height: 720 },
  },
});
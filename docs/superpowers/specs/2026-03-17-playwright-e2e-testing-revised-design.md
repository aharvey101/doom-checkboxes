# Playwright E2E Testing Implementation Design (Revised)

**Date:** 2026-03-17  
**Status:** Draft - Technical Review Applied  
**Type:** Infrastructure Enhancement  
**Previous Version:** Replaced due to technical issues identified in review

## Overview

Convert the test database infrastructure from incorrectly using Vitest for E2E testing to properly using Playwright for browser-based testing of the SpacetimeDB collaborative checkbox application. This leverages the existing Playwright setup in typescript-frontend rather than creating duplicate configurations.

## Problem Statement

The current implementation incorrectly migrated from Playwright to Vitest, creating a fundamental mismatch:

- **Vitest** is designed for unit testing individual modules
- **SpacetimeDB collaborative checkboxes** requires E2E testing for browser interactions, WebSocket connections, canvas rendering, and viewport navigation
- Current Vitest tests fail with WebSocket errors and browser API issues
- **Existing Playwright setup exists** in typescript-frontend but lacks proper integration with CI and environment management

## Requirements

### Functional Requirements
- **Automated CI testing** with local SpacetimeDB using existing Playwright setup
- **Manual staging/production testing** via documentation (no automation)
- **Proper test organization** leveraging existing typescript-frontend structure
- **SpacetimeDB state management** with cleanup between test runs

### Non-Functional Requirements
- **Leverage existing setup** - use current typescript-frontend Playwright configuration
- **Preserve CI reliability** - supplement existing tests, don't replace working pipeline
- **Handle collaborative state** - ensure test isolation for checkbox state
- **Clear developer workflow** - obvious distinction between test types

## Design Solution

### Architecture Overview

```
Testing Architecture (Revised):
├── E2E Testing (Playwright) - typescript-frontend/tests/ (existing location)
├── Unit Testing (Vitest) - typescript-frontend/test/ (preserve existing)  
├── Integration Tests - Root-level *.spec.js (organize and enhance)
└── Test Infrastructure - SpacetimeDB environments, state management
```

### Project Structure (Leveraging Existing Setup)

```
checkboxes-clean/
├── *.spec.js                      # Integration tests (enhance, don't move)
├── typescript-frontend/
│   ├── tests/                     # E2E tests (Playwright) - use existing
│   ├── test/                      # Unit tests (Vitest) - preserve
│   ├── playwright.config.js      # Create configuration here
│   └── package.json               # Already has Playwright scripts
├── scripts/
│   ├── test-db-manager.js         # Existing (enhance with state reset)
│   └── reset-test-state.js        # New - SpacetimeDB state cleanup
└── docs/
    └── TESTING.md                 # Manual testing documentation
```

**Key changes from previous design:**
- Use existing typescript-frontend Playwright setup instead of creating root-level duplication
- Keep integration tests at root level but enhance them
- Add proper SpacetimeDB state management
- Create playwright.config.js in typescript-frontend where dependencies exist

### Enhanced Test Organization

**Integration Tests (Root Level):**
- `test.spec.js` → Enhanced with state cleanup
- `test-panning.spec.js` → Enhanced with proper timeouts
- `test-mouse.spec.js` → Enhanced with environment awareness
- `test-fixed.spec.js` → Enhanced with state reset

**E2E Tests (typescript-frontend/tests/):**
- Create new comprehensive E2E tests that use the existing Playwright setup
- Focus on full user workflows and collaborative features
- Proper SpacetimeDB state management between tests

**Unit Tests (typescript-frontend/test/):**
- Keep existing Vitest tests for individual module testing
- These are correctly positioned and don't need changes

### Playwright Configuration (typescript-frontend/playwright.config.js)

```javascript
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '../', // Run integration tests from root level
  testMatch: ['*.spec.js', 'tests/**/*.spec.js'], // Both root and local tests
  
  timeout: 30000, // SpacetimeDB connection needs time
  expect: { timeout: 10000 },
  
  retries: 2, // Handle SpacetimeDB startup delays
  workers: 1, // Sequential execution for collaborative state management
  
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000',
    trace: 'on-first-retry',
    video: 'retain-on-failure',
  },
  
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
    }
  ],
  
  webServer: process.env.TEST_ENV === 'ci' ? {
    command: 'npm run dev',
    port: 8000,
    reuseExistingServer: true,
  } : undefined,
  
  // Global setup/teardown for SpacetimeDB state management
  globalSetup: './test-setup.js',
  globalTeardown: './test-teardown.js',
});
```

### SpacetimeDB State Management

**Create `scripts/reset-test-state.js`:**
```javascript
#!/usr/bin/env node
import { execSync } from 'child_process';

// Reset SpacetimeDB test database state
export async function resetTestState() {
  try {
    // Clear checkbox data in test database
    execSync('spacetime call clear_all_checkboxes', { 
      stdio: 'inherit',
      cwd: '../backend' 
    });
    console.log('✅ Test state reset successfully');
  } catch (error) {
    console.log('⚠️ Test state reset failed:', error.message);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  await resetTestState();
}
```

**Enhanced `scripts/test-db-manager.js`:**
Add state reset functionality to existing manager.

**Test Setup (`typescript-frontend/test-setup.js`):**
```javascript
import { resetTestState } from '../scripts/reset-test-state.js';

export default async function globalSetup() {
  // Ensure clean state before all tests
  await resetTestState();
}
```

### CI/CD Pipeline Integration (Supplement, Don't Replace)

**Update `.github/workflows/test-and-deploy.yml`:**

```yaml
# Keep existing unit test step
- name: Run unit tests
  run: |
    cd typescript-frontend
    TEST_ENV=ci npm run test:ci

# Add NEW parallel E2E test step  
- name: Install Playwright browsers
  run: |
    cd typescript-frontend
    npx playwright install --with-deps chromium

- name: Run E2E test suite
  run: |
    cd typescript-frontend  
    TEST_ENV=ci npm run test:e2e-playwright

# Keep existing coverage (for unit tests)
- name: Generate test coverage
  run: |
    cd typescript-frontend  
    TEST_ENV=ci npm run test:coverage

# Add E2E test results upload
- name: Upload E2E test results
  uses: actions/upload-artifact@v3
  if: always()
  with:
    name: playwright-report
    path: typescript-frontend/playwright-report/
```

### Package.json Updates (Enhance Existing)

**typescript-frontend/package.json** (already has Playwright scripts):
```json
{
  "scripts": {
    "dev": "vite",
    "test": "vitest", 
    "test:ci": "vitest run",
    "test:coverage": "vitest --coverage",
    "test:e2e-playwright": "playwright test --project=ci",
    "test:e2e-staging": "playwright test --project=staging", 
    "test:e2e-ui": "playwright test --ui",
    "test:e2e-headed": "playwright test --headed",
    "test:e2e-debug": "playwright test --debug"
  }
}
```

**Root package.json** (minimal changes):
```json
{
  "scripts": {
    "test": "echo 'Run tests from typescript-frontend directory' && exit 1",
    "test:e2e": "cd typescript-frontend && npm run test:e2e-playwright",
    "test:integration": "cd typescript-frontend && playwright test ../*.spec.js"
  }
}
```

### Documentation Strategy (`docs/TESTING.md`)

```markdown
# Testing Guide

## Test Types

### Unit Tests (Vitest)
```bash
cd typescript-frontend
npm run test              # Watch mode
npm run test:ci           # CI mode 
npm run test:coverage     # With coverage
```

### E2E Tests (Playwright)
```bash
cd typescript-frontend
npm run test:e2e-playwright    # All E2E tests
npm run test:e2e-debug         # Debug mode
npm run test:e2e-ui            # UI mode
```

### Integration Tests
```bash
cd typescript-frontend
npm run test:integration       # Root-level *.spec.js files
```

## Manual Environment Testing

### Staging Testing
1. Deploy to staging: `./scripts/deploy-staging.sh`
2. Set environment: `export STAGING_URL=https://staging-url.netlify.app`
3. Run tests: `cd typescript-frontend && npm run test:e2e-staging`
4. View report: `npx playwright show-report`

### Production Testing (Read-Only)
Similar to staging but with production URLs and read-only test patterns.

## Test Data Management

Tests automatically reset SpacetimeDB state between runs. For manual reset:
```bash
node scripts/reset-test-state.js
```
```

### Implementation Tasks (Revised)

1. **Create typescript-frontend/playwright.config.js with environment detection**
2. **Enhance existing root-level *.spec.js tests with state management**
3. **Create SpacetimeDB state reset functionality** 
4. **Add parallel E2E step to CI pipeline (preserve existing unit tests)**
5. **Create comprehensive testing documentation**
6. **Add test setup/teardown for state management**
7. **Validate complete pipeline with proper test isolation**

## Success Criteria

- ✅ CI pipeline runs both unit tests (Vitest) and E2E tests (Playwright) successfully
- ✅ Proper test isolation with SpacetimeDB state cleanup between runs
- ✅ Existing typescript-frontend Playwright setup enhanced, not replaced
- ✅ Integration tests at root level improved with environment awareness
- ✅ Manual staging/production testing documented and validated  
- ✅ Clear separation between unit, integration, and E2E test types

## Risk Mitigation

- **Leverage existing setup:** Build on working typescript-frontend Playwright configuration
- **Preserve CI reliability:** Add E2E tests as supplement, keep working unit test pipeline  
- **State isolation:** Explicit SpacetimeDB reset between tests prevents collaborative state issues
- **Gradual migration:** Enhance existing tests incrementally rather than wholesale replacement

## Technical Advantages of Revised Approach

1. **No duplicate dependencies:** Uses existing Playwright setup in typescript-frontend
2. **Proper working directory:** Configuration located where dependencies exist
3. **State management:** Explicit SpacetimeDB cleanup prevents test interference  
4. **CI preservation:** Supplements rather than replaces working test pipeline
5. **Clear organization:** Unit, integration, and E2E tests each have proper locations

This revised design addresses the technical issues identified in review while maintaining the core objective of fixing the Vitest/Playwright mismatch for proper E2E testing of the collaborative SpacetimeDB application.
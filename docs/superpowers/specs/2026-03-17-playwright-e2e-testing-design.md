# Playwright E2E Testing Implementation Design

**Date:** 2026-03-17  
**Status:** Approved  
**Type:** Infrastructure Enhancement

## Overview

Convert the test database infrastructure from using Vitest (unit testing) to Playwright (E2E testing) for proper browser-based testing of the SpacetimeDB collaborative checkbox application. This addresses the critical architectural error where unit tests were being used for end-to-end functionality testing.

## Problem Statement

The current implementation incorrectly migrated from Playwright to Vitest, creating a mismatch between testing needs and testing tools:

- **Vitest** is designed for unit testing individual modules
- **SpacetimeDB collaborative checkboxes** requires E2E testing for browser interactions, WebSocket connections, canvas rendering, and viewport navigation
- Current Vitest tests fail with WebSocket errors, timeout issues, and missing browser APIs
- Existing Playwright tests at project root work correctly but lack proper configuration

## Requirements

### Functional Requirements
- **Automated CI testing** with local SpacetimeDB using Playwright
- **Manual staging/production testing** via documentation (no automation)
- **Proper test organization** with clear separation between unit and E2E tests
- **Environment awareness** leveraging existing test database infrastructure

### Non-Functional Requirements
- **Minimal complexity** - simple configuration and maintenance
- **Reuse existing infrastructure** - leverage current SpacetimeDB setup scripts
- **Clear developer workflow** - obvious distinction between test types
- **Reliable CI integration** - stable automated testing

## Design Solution

### Architecture Overview

```
Testing Architecture:
├── E2E Testing (Playwright) - Browser automation, SpacetimeDB integration
├── Unit Testing (Vitest) - Individual module testing  
└── Test Infrastructure - SpacetimeDB environments, deployment scripts
```

### Project Structure

```
checkboxes-clean/
├── playwright.config.js           # Main Playwright configuration
├── e2e/                           # All E2E tests (moved from root)
│   ├── basic-functionality.spec.js    # Core app loading and initialization  
│   ├── checkbox-interactions.spec.js  # Clicking, state persistence
│   ├── viewport-navigation.spec.js    # Arrow keys, panning, grid updates
│   └── mouse-interactions.spec.js     # Mouse-based interactions
├── typescript-frontend/
│   ├── test/                      # Unit tests (Vitest) - keep existing
│   └── package.json               # Updated scripts for both test types
├── scripts/
│   └── test-db-manager.js         # Existing environment manager (no changes)
└── docs/
    └── TESTING.md                 # Manual testing documentation
```

### Configuration & Environment Management

**Playwright Configuration (`playwright.config.js`):**
- Environment detection via `process.env.TEST_ENV`
- Default to CI environment (localhost:8000 + localhost:3001)
- Support for staging/production URLs when needed
- Reasonable timeouts for SpacetimeDB connection delays
- Test parallelization and retry policies

**Environment Integration:**
- Leverage existing `scripts/test-db-manager.js` (no modifications required)
- CI automatically uses local SpacetimeDB via existing startup scripts
- Manual environment switching documented for staging/production testing

**Test Environment URLs:**
- **CI:** `http://localhost:8000` (local dev server) + `http://localhost:3001` (local SpacetimeDB)
- **Staging:** Documented manual process pointing to staging URLs
- **Production:** Documented manual process with read-only test guidelines

### CI/CD Pipeline Integration

**GitHub Actions Workflow Updates (`.github/workflows/test-and-deploy.yml`):**

1. **Replace Vitest E2E with Playwright E2E:**
   - Change `npm run test:ci` → `npm run test:e2e`
   - Remove `npm run test:coverage` (unit test coverage not relevant for E2E)
   - Add Playwright browser installation step

2. **Enhanced Test Flow:**
   ```yaml
   - name: Install Playwright browsers
     run: npx playwright install --with-deps
   
   - name: Run E2E test suite  
     run: npm run test:e2e
   
   - name: Upload test results
     uses: actions/upload-artifact@v3
     if: always()
     with:
       name: playwright-report
       path: playwright-report/
   ```

3. **Keep Existing Infrastructure:**
   - SpacetimeDB startup/deployment steps unchanged
   - Environment manager integration unchanged
   - Backend module build process unchanged

### Package.json Script Updates

**Root package.json:**
```json
{
  "scripts": {
    "test:e2e": "playwright test",
    "test:e2e:headed": "playwright test --headed", 
    "test:e2e:debug": "playwright test --debug",
    "test:e2e:ui": "playwright test --ui"
  }
}
```

**typescript-frontend/package.json:**
```json
{
  "scripts": {
    "test": "vitest",
    "test:unit": "vitest run",
    "test:unit:watch": "vitest --watch",
    "test:coverage": "vitest --coverage",
    "test:e2e": "cd .. && npm run test:e2e"
  }
}
```

### Test Migration Strategy

**Existing Tests Reorganization:**
- Move `test.spec.js` → `e2e/basic-functionality.spec.js`
- Move `test-panning.spec.js` → `e2e/viewport-navigation.spec.js`  
- Move `test-mouse.spec.js` → `e2e/mouse-interactions.spec.js`
- Move `test-fixed.spec.js` → `e2e/checkbox-interactions.spec.js`

**Test Content Updates:**
- Update hardcoded URLs to use environment variables
- Enhance test descriptions and organization
- Add proper test setup/teardown where needed
- Keep existing test logic (it works correctly)

### Documentation Strategy

**Create `docs/TESTING.md` covering:**

1. **Developer Workflow:**
   - Local development: `npm run test:e2e` (uses CI environment)
   - Unit testing: `cd typescript-frontend && npm run test:unit`
   - Debugging: `npm run test:e2e:debug`

2. **Manual Staging Testing:**
   ```bash
   # 1. Deploy to staging
   ./scripts/deploy-staging.sh
   
   # 2. Set environment variables
   export TEST_ENV=staging
   export PLAYWRIGHT_BASE_URL=https://staging-frontend-url.netlify.app
   
   # 3. Run tests
   npx playwright test
   
   # 4. View results
   npx playwright show-report
   ```

3. **Manual Production Testing:**
   - Similar process with production URLs
   - Warning about testing against live data
   - Recommended read-only test patterns

4. **Test Maintenance:**
   - Adding new E2E tests
   - Browser debugging techniques
   - Test data cleanup procedures

## Implementation Tasks

1. **Create Playwright configuration**
2. **Reorganize existing tests into /e2e directory**
3. **Update package.json scripts for both root and typescript-frontend**
4. **Modify CI/CD pipeline to use Playwright instead of Vitest**
5. **Create comprehensive testing documentation**
6. **Remove problematic Vitest E2E tests from typescript-frontend/test/**
7. **Test the complete pipeline with local SpacetimeDB**

## Success Criteria

- ✅ CI pipeline runs Playwright E2E tests against local SpacetimeDB successfully
- ✅ Clear separation between unit tests (Vitest) and E2E tests (Playwright)  
- ✅ Existing test functionality preserved with better organization
- ✅ Manual staging/production testing documented and validated
- ✅ Developer workflow simplified with obvious test type distinctions
- ✅ Test infrastructure reuses existing SpacetimeDB environment management

## Risk Mitigation

- **Low implementation risk:** Existing tests work, just need reorganization
- **Environment isolation:** Existing test-db-manager.js handles environment switching safely
- **Rollback plan:** Original test files preserved until verification complete
- **Testing approach:** Validate each component (config, tests, CI) independently

## Future Considerations

- **Unit test expansion:** Add proper Vitest unit tests for individual TypeScript modules
- **Test data management:** Enhanced test data cleanup for staging/production
- **Performance testing:** Potential addition of Playwright performance tests
- **Visual regression:** Future consideration for visual diff testing

This design maintains the robust test database infrastructure while fixing the fundamental testing framework mismatch, resulting in reliable E2E testing for the SpacetimeDB collaborative checkbox application.
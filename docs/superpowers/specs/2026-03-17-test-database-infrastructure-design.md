# Test Database Infrastructure Design

**Date:** 2026-03-17  
**Status:** Draft  
**Context:** Implement proper test database isolation and CI/CD automation for SpacetimeDB collaborative checkbox application

## Problem Statement

Our current test infrastructure has several critical issues:
1. Tests hardcoded to various endpoints (localhost:5174, localhost:3001) 
2. Test framework conflicts (Playwright vs Vitest syntax mixing)
3. Shared production database contamination risk
4. No automated SpacetimeDB deployment on merge to main
5. No isolated test environment for CI/CD pipelines
6. No staging environment for manual testing and demos

## Solution Overview

Implement a three-tier database infrastructure with automated deployment:

```
┌─ CI/CD Pipeline ─────────────────────────┐
│  ├─ Spin up local SpacetimeDB           │
│  ├─ Run test suite against localhost    │  
│  ├─ Deploy to staging on PR merge       │
│  └─ Deploy to production on main merge  │
└─────────────────────────────────────────┘

┌─ Staging Environment ────────────────────┐
│  ├─ collaborative-checkboxes-staging    │
│  ├─ Used for manual testing/demos       │
│  ├─ Deployed from main branch           │
│  └─ Daily reset via scheduled job       │
└─────────────────────────────────────────┘

┌─ Production Environment ─────────────────┐
│  ├─ collaborative-checkboxes-prod       │
│  ├─ Deployed from main branch           │
│  └─ Never touched by tests              │
└─────────────────────────────────────────┘
```

## Architecture Components

### 1. Database Configurations

**Three SpacetimeDB configuration files:**

- **`backend/spacetime.json`** → Production Environment
  ```json
  {
    "server": "https://maincloud.spacetimedb.com",
    "database": "collaborative-checkboxes-prod"
  }
  ```

- **`backend/spacetime.staging.json`** → Staging Environment  
  ```json
  {
    "server": "https://maincloud.spacetimedb.com",
    "database": "collaborative-checkboxes-staging"
  }
  ```

- **`backend/spacetime.ci.json`** → CI/CD Environment
  ```json
  {
    "server": "http://localhost:3001",
    "database": "checkboxes-ci-test"
  }
  ```

### 2. Test Environment Manager

**`scripts/test-db-manager.js`** - Node.js script that:
- Detects which environment to use based on environment variables
- Switches SpacetimeDB configuration files
- Manages local SpacetimeDB lifecycle for CI
- Coordinates test database cleanup

**Environment Detection:**
```javascript
const environment = process.env.TEST_ENV || 'ci';
// 'ci' = local SpacetimeDB instance
// 'staging' = staging database  
// 'production' = production database (read-only tests only)
```

### 3. CI/CD Pipeline Integration

**GitHub Actions Workflow (`.github/workflows/test-and-deploy.yml`):**

**On Pull Requests:**
1. Spin up local SpacetimeDB instance
2. Deploy backend module to localhost
3. Run full test suite against localhost
4. Generate test reports
5. Teardown local instance

**On Merge to Main:**
1. Run full test suite (as above)
2. Deploy backend to staging environment
3. Deploy backend to production environment  
4. Update frontend configuration
5. Deploy frontend to Netlify

### 4. Staging Environment Management

**Daily Reset Automation:**
- GitHub Actions scheduled workflow (runs daily at 2 AM UTC)
- Connects to staging database
- Truncates all tables (preserves schema)
- Logs reset completion

**Manual Reset Option:**
- `npm run reset:staging` command for immediate cleanup
- Useful before demos or major testing sessions

### 5. Test Suite Standardization

**Framework Consolidation:**
- Migrate all tests to Vitest (remove Playwright syntax conflicts)
- Standardize test URLs via environment configuration
- Add proper test isolation and cleanup

**Test Categories:**
- **Unit Tests:** Component logic, pure functions (no database)
- **Integration Tests:** Database operations, reducer calls  
- **E2E Tests:** Full application flows via staging environment

### 6. SpacetimeDB Deployment Automation

**Automated Backend Deployment:**
- Staging deployment on every main branch push
- Production deployment on every main branch push  
- Rollback capability via git tags
- Schema migration handling

**Backend Deployment Flow:**
```bash
# 1. Build and validate module
cargo build --release --target wasm32-unknown-unknown

# 2. Deploy to staging
spacetime publish --name collaborative-checkboxes-staging

# 3. Run smoke tests against staging
npm run test:staging:smoke

# 4. Deploy to production (if staging tests pass)
spacetime publish --name collaborative-checkboxes-prod
```

**Frontend Deployment Coordination:**
- Update TypeScript frontend configuration after backend deployment
- Regenerate SpacetimeDB bindings if schema changes
- Deploy frontend to Netlify with new backend references

## Implementation Plan

### Phase 1: Database Configuration Setup
1. Create three SpacetimeDB configuration files
2. Create staging database (`collaborative-checkboxes-staging`)
3. Implement test environment manager script
4. Add environment switching to test scripts

### Phase 2: CI/CD Pipeline Integration  
1. Create GitHub Actions workflow for PR testing
2. Add local SpacetimeDB setup to CI environment
3. Implement automated test database lifecycle
4. Add test reporting and artifacts

### Phase 3: Deployment Automation
1. Add GitHub Actions workflow for main branch deployment
2. Implement staging deployment automation
3. Implement production deployment automation  
4. Add frontend configuration updates
5. Add rollback procedures

### Phase 4: Test Suite Modernization
1. Migrate all tests from Playwright syntax to Vitest
2. Standardize test environment configuration
3. Add proper test isolation and cleanup
4. Create comprehensive test coverage

### Phase 5: Staging Environment Management
1. Implement daily reset automation
2. Add manual reset commands
3. Create staging environment monitoring
4. Add staging-specific test suites

## File Structure Changes

```
checkboxes-clean/
├── backend/
│   ├── spacetime.json              # Production config
│   ├── spacetime.staging.json      # Staging config  
│   └── spacetime.ci.json          # CI config
├── scripts/
│   ├── test-db-manager.js         # Environment manager
│   ├── deploy-staging.sh          # Staging deployment
│   ├── deploy-production.sh       # Production deployment
│   └── reset-staging.js           # Staging cleanup
├── .github/workflows/
│   ├── test-and-deploy.yml        # Main CI/CD pipeline
│   └── daily-staging-reset.yml    # Scheduled staging cleanup
└── typescript-frontend/
    ├── tests/                     # Standardized tests
    └── test-config.js             # Environment-aware test config
```

## Environment Variables

**Required for CI/CD:**
```bash
SPACETIMEDB_TOKEN=<spacetime-cli-token>    # For automated deployments
TEST_ENV=ci|staging|production             # Test environment selection
NETLIFY_TOKEN=<netlify-deployment-token>   # For frontend deployment
```

**Required for Local Development:**
```bash
TEST_ENV=ci                               # Use local SpacetimeDB
SPACETIME_LOCAL_PATH=/path/to/spacetime   # Local SpacetimeDB binary
```

## Success Criteria

1. **Test Isolation:** No test contamination between runs
2. **CI/CD Reliability:** All tests pass consistently in pipeline  
3. **Deployment Automation:** Backend and frontend deploy automatically on main merge
4. **Staging Environment:** Clean staging environment reset daily
5. **Developer Experience:** Simple commands for local testing and environment switching

## Rollout Strategy

1. Implement in feature branch with comprehensive testing
2. Validate against current production environment (read-only)
3. Deploy staging infrastructure first
4. Gradually migrate test suite to new infrastructure
5. Enable automated deployment after manual validation
6. Monitor for 1 week before considering complete

## Risk Mitigation

**Database Loss Prevention:**
- Production database never touched by automated scripts
- Staging database backed up before major changes
- Rollback procedures tested and documented

**CI/CD Failure Handling:**
- Deployment failures don't affect existing environments
- Local SpacetimeDB startup failures fail fast
- Manual deployment procedures documented as backup

**Test Infrastructure Reliability:**
- Multiple retry attempts for database operations
- Graceful degradation if staging environment unavailable
- Local fallback for critical test runs

## Future Considerations

1. **Performance Testing:** Separate performance test environment
2. **Load Testing:** Dedicated load testing infrastructure  
3. **Multi-Region:** Staging environments in multiple regions
4. **Schema Versioning:** Automated schema migration testing
5. **Monitoring:** Database and application monitoring integration
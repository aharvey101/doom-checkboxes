#!/usr/bin/env node

/**
 * Playwright global teardown for SpacetimeDB collaborative checkbox tests
 * Cleans up test environment
 */
async function globalTeardown() {
  console.log('🧹 Starting Playwright global teardown...');
  
  try {
    // For now, teardown is minimal since setup only resets test state
    // In the future, this could include cleanup tasks like stopping test servers
    
    console.log('✅ Playwright global teardown completed successfully');
  } catch (error) {
    console.error('⚠️ Global teardown encountered an issue:', error.message);
    // Don't throw here - teardown failures shouldn't fail the test run
    console.log('Test results are still valid despite teardown issues');
  }
}

export default globalTeardown;
#!/usr/bin/env node
import { resetTestState } from './scripts/reset-test-state.js';

/**
 * Global teardown for Playwright tests - cleanup SpacetimeDB state
 */
export default async function globalTeardown() {
  console.log('🧹 Cleaning up test environment...');
  
  // Reset SpacetimeDB state after all tests
  try {
    await resetTestState();
    console.log('✅ Test environment cleanup complete');
  } catch (error) {
    console.log('⚠️ Test cleanup failed:', error.message);
    // Don't fail teardown - cleanup is best effort
  }
}

// Allow direct execution
if (import.meta.url === `file://${process.argv[1]}`) {
  await globalTeardown();
}
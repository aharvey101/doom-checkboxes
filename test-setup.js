#!/usr/bin/env node
import { resetTestState } from './scripts/reset-test-state.js';
import { execSync } from 'child_process';

/**
 * Global setup for Playwright tests - ensures clean SpacetimeDB state
 */
export default async function globalSetup() {
  console.log('🧪 Setting up test environment...');
  
  // Ensure SpacetimeDB is available for state reset
  try {
    execSync('spacetime version list', { stdio: 'ignore' });
  } catch (error) {
    console.log('⚠️ SpacetimeDB CLI not available - skipping state reset');
    return;
  }
  
  // Reset SpacetimeDB state before all tests
  try {
    await resetTestState();
    console.log('✅ Test environment setup complete');
  } catch (error) {
    console.log('⚠️ Test state reset failed during setup:', error.message);
    // Don't fail setup - tests may still work
  }
}

// Allow direct execution
if (import.meta.url === `file://${process.argv[1]}`) {
  await globalSetup();
}
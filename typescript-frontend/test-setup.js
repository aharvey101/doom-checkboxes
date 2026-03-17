#!/usr/bin/env node
import path from 'path';

/**
 * Playwright global setup for SpacetimeDB collaborative checkbox tests
 * Ensures clean database state before test runs
 */
async function globalSetup() {
  console.log('🚀 Starting Playwright global setup...');
  
  try {
    // Import the reset test state function
    const modulePath = path.resolve('../scripts/reset-test-state.js');
    const { resetTestState } = await import(modulePath);

    // Reset test data to ensure clean state
    console.log('📋 Resetting test state...');
    const success = await resetTestState();
    
    if (!success) {
      throw new Error('Failed to reset test state');
    }
    
    console.log('✅ Playwright global setup completed successfully');
  } catch (error) {
    console.error('❌ Global setup failed:', error.message);
    throw error;
  }
}

export default globalSetup;
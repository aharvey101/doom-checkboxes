#!/usr/bin/env node
import { execSync } from 'child_process';
import path from 'path';
import fs from 'fs';

/**
 * Reset SpacetimeDB test database state for clean test runs
 */
export async function resetTestState() {
  try {
    const backendDir = path.resolve(path.dirname(import.meta.url.replace('file://', '')), '../backend');
    
    // Safety check - ensure we're not accidentally clearing production data
    const configPath = path.join(backendDir, 'spacetime.json');
    if (fs.existsSync(configPath)) {
      const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
      if (config.database && config.database.includes('prod')) {
        throw new Error('Safety check failed: Cannot reset production database. Please use local or staging configuration.');
      }
    }
    
    // Clear all checkbox data in test database
    execSync('spacetime call clear_all_checkboxes', { 
      stdio: 'inherit',
      cwd: backendDir
    });
    console.log('✅ Test state reset successfully');
    return true;
  } catch (error) {
    console.log('⚠️ Test state reset failed:', error.message);
    return false;
  }
}

// Allow direct execution
if (import.meta.url === `file://${process.argv[1]}`) {
  const success = await resetTestState();
  process.exit(success ? 0 : 1);
}
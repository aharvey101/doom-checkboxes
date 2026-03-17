#!/usr/bin/env node
import fs from 'fs-extra';
import { execSync } from 'child_process';
import path from 'path';

const BACKEND_DIR = path.resolve('../backend');
const CONFIGS = {
  production: 'spacetime.production.json',
  staging: 'spacetime.staging.json', 
  ci: 'spacetime.ci.json'
};

class TestDatabaseManager {
  constructor() {
    this.environment = process.env.TEST_ENV || 'ci';
    this.backupPath = path.join(BACKEND_DIR, 'spacetime.json.backup');
  }

  async switchEnvironment(env) {
    if (!CONFIGS[env]) {
      throw new Error(`Unknown environment: ${env}`);
    }

    const configPath = path.join(BACKEND_DIR, CONFIGS[env]);
    const mainConfigPath = path.join(BACKEND_DIR, 'spacetime.json');

    // Backup current config
    if (await fs.pathExists(mainConfigPath)) {
      await fs.copy(mainConfigPath, this.backupPath);
    }

    // Switch to new config
    await fs.copy(configPath, mainConfigPath);
    console.log(`✓ Switched to ${env} environment`);
  }

  async restoreEnvironment() {
    if (await fs.pathExists(this.backupPath)) {
      const mainConfigPath = path.join(BACKEND_DIR, 'spacetime.json');
      await fs.copy(this.backupPath, mainConfigPath);
      await fs.remove(this.backupPath);
      console.log('✓ Restored original environment');
    }
  }

  async startLocalSpacetimeDB() {
    try {
      // Check if SpacetimeDB is already running
      execSync('curl -f http://localhost:3001/health', { stdio: 'ignore' });
      console.log('✓ SpacetimeDB already running on localhost:3001');
      return;
    } catch {
      // Not running, start it
    }

    console.log('Starting local SpacetimeDB...');
    
    // Use spawn for proper process management
    const { spawn } = await import('child_process');
    const spacetimeProcess = spawn('spacetime', ['start', '--listen', '0.0.0.0:3001'], {
      detached: true,
      stdio: ['ignore', 'ignore', 'ignore']
    });
    
    spacetimeProcess.unref(); // Allow parent to exit
    
    // Save PID for later cleanup
    await fs.writeFile('/tmp/spacetime-test.pid', spacetimeProcess.pid.toString());
    
    // Wait for startup
    let retries = 10;
    while (retries > 0) {
      try {
        execSync('curl -f http://localhost:3001/health', { stdio: 'ignore' });
        console.log('✓ SpacetimeDB started successfully');
        return;
      } catch {
        await new Promise(resolve => setTimeout(resolve, 1000));
        retries--;
      }
    }
    
    throw new Error('Failed to start SpacetimeDB after 10 seconds');
  }

  async stopLocalSpacetimeDB() {
    try {
      execSync('pkill -f "spacetimedb start"');
      console.log('✓ Stopped local SpacetimeDB');
    } catch {
      console.log('No SpacetimeDB process to stop');
    }
  }

  async resetTestData() {
    const { resetTestState } = await import('./reset-test-state.js');
    const success = await resetTestState();
    if (success) {
      console.log('✓ Test data reset completed');
    } else {
      console.log('⚠️ Test data reset failed');
      throw new Error('Failed to reset test data');
    }
  }
}

// CLI interface
const command = process.argv[2];
const manager = new TestDatabaseManager();

switch (command) {
  case 'switch':
    const env = process.argv[3];
    await manager.switchEnvironment(env);
    break;
  case 'restore':
    await manager.restoreEnvironment();
    break;
  case 'start-local':
    await manager.startLocalSpacetimeDB();
    break;
  case 'stop-local':
    await manager.stopLocalSpacetimeDB();
    break;
  case 'reset-data':
    await manager.resetTestData();
    break;
  default:
    console.log('Usage: node test-db-manager.js [switch|restore|start-local|stop-local|reset-data] [env]');
    process.exit(1);
}
// test-config.js
import { readFileSync } from 'fs';
import { resolve } from 'path';

export class TestEnvironmentConfig {
  constructor() {
    this.environment = process.env.TEST_ENV || 'ci';
    this.config = this.loadSpacetimeConfig();
  }

  loadSpacetimeConfig() {
    const configFile = this.getConfigFile();
    const configPath = resolve(`../backend/${configFile}`);
    
    try {
      const content = readFileSync(configPath, 'utf8');
      const config = JSON.parse(content);
      
      return {
        server: config.server,
        database: config.database,
        baseUrl: this.getBaseUrl()
      };
    } catch (error) {
      throw new Error(`Failed to load SpacetimeDB config: ${error.message}`);
    }
  }

  getConfigFile() {
    switch (this.environment) {
      case 'production':
        return 'spacetime.json';
      case 'staging':
        return 'spacetime.staging.json';
      case 'ci':
        return 'spacetime.ci.json';
      default:
        throw new Error(`Unknown test environment: ${this.environment}`);
    }
  }

  getBaseUrl() {
    switch (this.environment) {
      case 'production':
        return 'https://checkbox-grid-100x100.netlify.app';
      case 'staging':
        return 'https://checkbox-grid-staging.netlify.app';
      case 'ci':
        return 'http://localhost:5174';
      default:
        throw new Error(`Unknown test environment: ${this.environment}`);
    }
  }

  getTestTimeout() {
    switch (this.environment) {
      case 'ci':
        return 10000; // Local tests can be faster
      case 'staging':
      case 'production':
        return 30000; // Remote tests need more time
      default:
        return 15000;
    }
  }

  getDatabaseConfig() {
    return {
      server: this.config.server,
      database: this.config.database
    };
  }

  shouldSkipE2E() {
    // Skip E2E tests in CI by default (can be overridden)
    return this.environment === 'ci' && !process.env.RUN_E2E_TESTS;
  }
}

export const testConfig = new TestEnvironmentConfig();
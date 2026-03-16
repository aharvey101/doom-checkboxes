import { test, expect } from '@playwright/test';

test.describe('SpacetimeDB Configuration Fix', () => {
  test('should connect to correct server when configuration parameters are provided', async ({ page }) => {
    await page.goto('/');
    
    // Monitor console logs to verify connection parameters
    const connectionLogs: string[] = [];
    page.on('console', msg => {
      if (msg.text().includes('🔌 Connecting to SpacetimeDB') || 
          msg.text().includes('📍 Database address')) {
        connectionLogs.push(msg.text());
      }
    });
    
    // The HTML initializes with localhost settings
    // But we expect the connection to use those settings, not hardcoded production
    await page.click('#connectBtn');
    
    // Wait for connection attempt
    await page.waitForTimeout(2000);
    
    // Verify that connection logs show localhost settings being used
    const serverLog = connectionLogs.find(log => log.includes('🔌 Connecting to SpacetimeDB'));
    const databaseLog = connectionLogs.find(log => log.includes('📍 Database address'));
    
    // This test will fail because currently the database client ignores
    // the constructor parameters and uses hardcoded production values
    expect(serverLog).toContain('http://localhost:3000');
    expect(databaseLog).toContain('checkboxes-local-demo');
  });
});
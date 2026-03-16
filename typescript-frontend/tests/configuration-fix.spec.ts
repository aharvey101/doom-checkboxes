import { test, expect } from '@playwright/test';

test.describe('SpacetimeDB Configuration Fix', () => {
  test('should connect using constructor parameters instead of hardcoded defaults', async ({ page }) => {
    await page.goto('/');
    
    // Monitor console logs to verify connection parameters
    const connectionLogs: string[] = [];
    page.on('console', msg => {
      if (msg.text().includes('🔌 Connecting to SpacetimeDB') || 
          msg.text().includes('📍 Database address')) {
        connectionLogs.push(msg.text());
      }
    });
    
    // Click connect button to initiate connection
    await page.click('#connectBtn');
    
    // Wait for connection attempt
    await page.waitForTimeout(2000);
    
    // Verify that connection logs show constructor parameters being used
    const serverLog = connectionLogs.find(log => log.includes('🔌 Connecting to SpacetimeDB'));
    const databaseLog = connectionLogs.find(log => log.includes('📍 Database address'));
    
    // The HTML now initializes with production settings
    // Verify that connection logs show production settings being used properly
    expect(serverLog).toContain('https://maincloud.spacetimedb.com');
    expect(databaseLog).toContain('c200d12d98ef0c856a8ba926a0f711a75ef243fe097a24f6c26836f0ff2215a0');
  });
});
import { test, expect } from '@playwright/test';

test.describe('SpacetimeDB CSP Fix', () => {
  test('should connect to SpacetimeDB without CSP violations', async ({ page }) => {
    await page.goto('/');
    
    // Monitor for CSP violations
    const cspErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error' && msg.text().includes('Content Security Policy')) {
        cspErrors.push(msg.text());
      }
    });
    
    // Monitor for connection logs
    const connectionLogs: string[] = [];
    page.on('console', msg => {
      if (msg.text().includes('🔌 Connecting') || 
          msg.text().includes('✅ Connected') || 
          msg.text().includes('❌ Connection error')) {
        connectionLogs.push(msg.text());
      }
    });
    
    // Attempt SpacetimeDB connection
    await page.click('#connectBtn');
    
    // Wait for connection attempt
    await page.waitForTimeout(5000);
    
    // Verify no CSP violations occurred
    expect(cspErrors).toHaveLength(0);
    
    // Verify connection attempt was made (should see at least connecting log)
    const connectingLog = connectionLogs.find(log => log.includes('🔌 Connecting'));
    expect(connectingLog).toBeTruthy();
    
    console.log('Connection logs:', connectionLogs);
    console.log('CSP errors:', cspErrors);
  });
});
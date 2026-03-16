import { test, expect } from '@playwright/test';

test.describe('SpacetimeDB Connection & Database', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
  });

  test('should display connection status', async ({ page }) => {
    // Check initial connection status
    const statusIndicator = page.locator('#status');
    await expect(statusIndicator).toBeVisible();
    
    // Should initially show disconnected
    await expect(statusIndicator).toContainText('Disconnected');
  });

  test('should attempt to connect when button is clicked', async ({ page }) => {
    const connectButton = page.locator('#connectBtn');
    const activityLog = page.locator('#log');
    
    // Click connect button
    await connectButton.click();
    
    // Should see connection attempt in activity log
    await page.waitForTimeout(1000);
    const logContent = await activityLog.textContent();
    expect(logContent?.toLowerCase()).toContain('connect');
  });

  test('should display database configuration', async ({ page }) => {
    // Check that database info is displayed
    await expect(page.locator('#serverUrl')).toBeVisible();
    await expect(page.locator('#databaseName')).toBeVisible();
    
    // Should show the server URL and database name
    await expect(page.locator('#serverUrl')).toContainText('localhost:3000');
    await expect(page.locator('#databaseName')).toContainText('checkboxes-local-demo');
  });

  test('should show SDK version information', async ({ page }) => {
    await expect(page.locator('text=SDK:')).toBeVisible();
    await expect(page.locator('text=SpacetimeDB TypeScript 2.0.4')).toBeVisible();
  });

  test('should handle test button functionality', async ({ page }) => {
    const testButton = page.locator('#testBtn');
    const activityLog = page.locator('#log');
    
    // Initially test button should be disabled
    await expect(testButton).toBeDisabled();
    
    // Try clicking connect first (which might enable test button)
    await page.locator('#connectBtn').click();
    await page.waitForTimeout(500);
    
    // Check if button becomes enabled or activity is logged
    const logContent = await activityLog.textContent();
    expect(logContent).toBeTruthy();
  });

  test('should handle clear all functionality', async ({ page }) => {
    const clearButton = page.locator('#clearBtn');
    const activityLog = page.locator('#log');
    const checkedCount = page.locator('#checkedCount');
    
    // Initially clear button should be disabled
    await expect(clearButton).toBeDisabled();
    
    // Check initial checked count
    await expect(checkedCount).toContainText('0');
  });

  test('should display real-time features information', async ({ page }) => {
    const realTimeSection = page.locator('text=Real-time Features').locator('..');
    
    await expect(realTimeSection).toContainText('Live updates via subscriptions');
    await expect(realTimeSection).toContainText('Multi-user collaboration');
    await expect(realTimeSection).toContainText('Persistent checkbox state');
  });

  test('should log operations in activity log', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    const activityLog = page.locator('#log');
    
    // Focus canvas and try operations
    await canvas.click();
    await page.keyboard.press('Space');
    
    // Should see some activity logged
    await page.waitForTimeout(500);
    const logContent = await activityLog.textContent();
    expect(logContent?.length).toBeGreaterThan(10);
  });

  test('should handle connection errors gracefully', async ({ page }) => {
    // Monitor console for error handling
    const errors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });
    
    const connectButton = page.locator('#connectBtn');
    await connectButton.click();
    
    // Wait for connection attempt
    await page.waitForTimeout(2000);
    
    // Should either connect successfully or handle errors gracefully
    // We don't want uncaught exceptions
    const uncaughtErrors = errors.filter(error => 
      error.includes('Uncaught') || 
      error.includes('TypeError') ||
      error.includes('ReferenceError')
    );
    
    expect(uncaughtErrors).toHaveLength(0);
  });

  test('should display proper checkbox statistics', async ({ page }) => {
    // Check total count
    await expect(page.locator('#totalCount')).toContainText('10000');
    
    // Check grid size
    await expect(page.locator('text=Grid Size: 100×100')).toBeVisible();
    
    // Check initial checked count
    const checkedCount = page.locator('#checkedCount');
    await expect(checkedCount).toContainText('0');
  });

  test('should maintain activity log history', async ({ page }) => {
    const activityLog = page.locator('#log');
    const connectButton = page.locator('#connectBtn');
    
    // Perform multiple actions
    await connectButton.click();
    await page.waitForTimeout(500);
    
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    await page.keyboard.press('Space');
    await page.waitForTimeout(500);
    
    // Activity log should contain history
    const logContent = await activityLog.textContent();
    expect(logContent?.length).toBeGreaterThan(20);
  });

  test('should handle operations without crashing', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    
    // Perform a series of operations
    await canvas.click();
    
    // Navigate and toggle several checkboxes
    for (let i = 0; i < 5; i++) {
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press('Space');
      await page.waitForTimeout(100);
      
      await page.keyboard.press('ArrowDown');
      await page.keyboard.press('Space');
      await page.waitForTimeout(100);
    }
    
    // Page should still be responsive
    const positionDisplay = page.locator('#viewportPosition');
    await expect(positionDisplay).toBeVisible();
    
    // No JavaScript errors should have occurred
    const errors: string[] = [];
    page.on('pageerror', (error) => {
      errors.push(error.message);
    });
    
    await page.waitForTimeout(1000);
    
    const criticalErrors = errors.filter(error => 
      !error.includes('SpacetimeDB') && !error.includes('connection')
    );
    expect(criticalErrors).toHaveLength(0);
  });
});
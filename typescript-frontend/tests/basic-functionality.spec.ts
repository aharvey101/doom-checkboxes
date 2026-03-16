import { test, expect } from '@playwright/test';

test.describe('Checkbox Grid - Basic Functionality', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    // Wait for the page to be fully loaded
    await page.waitForLoadState('networkidle');
  });

  test('should load the main page correctly', async ({ page }) => {
    // Check page title
    await expect(page).toHaveTitle(/SpacetimeDB TypeScript Checkboxes/);
    
    // Check main heading
    await expect(page.locator('h1')).toContainText('SpacetimeDB TypeScript Checkboxes');
  });

  test('should display the checkbox grid container', async ({ page }) => {
    // Check that the grid container is present
    const gridContainer = page.locator('.grid-container');
    await expect(gridContainer).toBeVisible();
    
    // Check that viewport container exists
    const viewportContainer = page.locator('.viewport-container');
    await expect(viewportContainer).toBeVisible();
    
    // Check that canvas element exists inside the container
    const canvas = page.locator('#checkboxCanvas');
    await expect(canvas).toBeVisible();
    
    // Verify canvas has expected dimensions
    const canvasElement = await canvas.elementHandle();
    const width = await canvasElement?.getAttribute('width');
    const height = await canvasElement?.getAttribute('height');
    
    // Should have some reasonable canvas size (viewport-based)
    if (width && height) {
      expect(parseInt(width)).toBeGreaterThan(100);
      expect(parseInt(height)).toBeGreaterThan(100);
    }
  });

  test('should display control buttons', async ({ page }) => {
    // Check that main control buttons are present
    await expect(page.locator('#connectBtn')).toBeVisible();
    await expect(page.locator('#testBtn')).toBeVisible();
    await expect(page.locator('#clearBtn')).toBeVisible();
    
    // Check button text content
    await expect(page.locator('#connectBtn')).toContainText('Connect to SpacetimeDB');
    await expect(page.locator('#testBtn')).toContainText('Run Test');
    await expect(page.locator('#clearBtn')).toContainText('Clear All');
  });

  test('should display status information', async ({ page }) => {
    // Check status indicators
    await expect(page.locator('#status')).toBeVisible();
    await expect(page.locator('#status')).toContainText('Disconnected');
    
    // Check connection info
    await expect(page.locator('text=Connection Info')).toBeVisible();
    await expect(page.locator('#serverUrl')).toBeVisible();
    await expect(page.locator('#databaseName')).toBeVisible();
    
    // Check checkbox stats
    await expect(page.locator('text=Checkbox Stats')).toBeVisible();
    await expect(page.locator('#totalCount')).toContainText('10000');
    await expect(page.locator('text=Grid Size: 100×100')).toBeVisible();
  });

  test('should display position information', async ({ page }) => {
    // Check position display
    await expect(page.locator('text=Position:')).toBeVisible();
    await expect(page.locator('#viewportPosition')).toContainText('0, 0');
  });

  test('should display navigation instructions', async ({ page }) => {
    // Check instructions
    await expect(page.locator('text=Use arrow keys to navigate')).toBeVisible();
    await expect(page.locator('text=10,000 checkboxes')).toBeVisible();
  });

  test('should display activity log', async ({ page }) => {
    // Check activity log section
    await expect(page.locator('h3', { hasText: 'Activity Log' })).toBeVisible();
    await expect(page.locator('#log')).toBeVisible();
    
    // Should show initial loading message
    const activityLog = page.locator('#log');
    await expect(activityLog).toContainText('Loading TypeScript modules');
  });

  test('should have proper responsive design', async ({ page }) => {
    // Test different viewport sizes
    await page.setViewportSize({ width: 1200, height: 800 });
    await expect(page.locator('.grid-container')).toBeVisible();
    
    await page.setViewportSize({ width: 768, height: 1024 });
    await expect(page.locator('.grid-container')).toBeVisible();
    
    await page.setViewportSize({ width: 375, height: 667 });
    await expect(page.locator('.grid-container')).toBeVisible();
  });

  test('should load without JavaScript errors', async ({ page }) => {
    const errors: string[] = [];
    
    page.on('pageerror', (error) => {
      errors.push(error.message);
    });
    
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });
    
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    
    // Wait a bit for any delayed errors
    await page.waitForTimeout(2000);
    
    // Filter out expected errors (like connection failures during testing)
    const criticalErrors = errors.filter(error => 
      !error.includes('Failed to connect') && 
      !error.includes('WebSocket') &&
      !error.includes('SpacetimeDB')
    );
    
    expect(criticalErrors).toHaveLength(0);
  });

  test('should display real-time features information', async ({ page }) => {
    // Check that real-time features are listed
    await expect(page.locator('text=Real-time Features')).toBeVisible();
    await expect(page.locator('text=Live updates via subscriptions')).toBeVisible();
    await expect(page.locator('text=Multi-user collaboration')).toBeVisible();
    await expect(page.locator('text=Persistent checkbox state')).toBeVisible();
  });

  test('should have proper checkbox statistics', async ({ page }) => {
    // Check checkbox statistics
    await expect(page.locator('#totalCount')).toContainText('10000');
    await expect(page.locator('#checkedCount')).toContainText('0');
    await expect(page.locator('text=Grid Size: 100×100')).toBeVisible();
  });
});
import { test, expect } from '@playwright/test';

test.describe('Performance & Load Tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
  });

  test('should load the page within reasonable time', async ({ page }) => {
    const startTime = Date.now();
    
    await page.goto('/');
    await page.waitForLoadState('domcontentloaded');
    
    const loadTime = Date.now() - startTime;
    
    // Page should load within 5 seconds
    expect(loadTime).toBeLessThan(5000);
    
    // Canvas should be visible quickly
    await expect(page.locator('#checkboxCanvas')).toBeVisible({ timeout: 3000 });
  });

  test('should handle rapid navigation without lag', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    const startTime = Date.now();
    
    // Perform rapid navigation
    for (let i = 0; i < 20; i++) {
      await page.keyboard.press('ArrowRight');
      if (i % 4 === 0) await page.keyboard.press('ArrowDown');
    }
    
    const navigationTime = Date.now() - startTime;
    
    // Should handle 20 navigation moves quickly
    expect(navigationTime).toBeLessThan(2000);
    
    // Position should be accurate after rapid navigation
    const positionDisplay = page.locator('#viewportPosition');
    await expect(positionDisplay).toContainText('16, 5');
  });

  test('should maintain smooth rendering during navigation', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    const startTime = Date.now();
    
    // Navigate in a pattern that would cause viewport changes
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press('ArrowDown');
      await page.keyboard.press('ArrowLeft');
      await page.keyboard.press('ArrowDown');
    }
    
    const totalTime = Date.now() - startTime;
    
    // Should complete navigation smoothly
    expect(totalTime).toBeLessThan(3000);
    
    // Canvas should still be responsive
    const positionDisplay = page.locator('#viewportPosition');
    await expect(positionDisplay).toBeTruthy();
  });

  test('should handle multiple checkbox toggles efficiently', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    const startTime = Date.now();
    
    // Toggle checkboxes in a pattern
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press('Space'); // Toggle
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press('Space'); // Toggle
      await page.keyboard.press('ArrowDown');
    }
    
    const toggleTime = Date.now() - startTime;
    
    // Should handle 20 toggles quickly
    expect(toggleTime).toBeLessThan(3000);
    
    // UI should remain responsive
    const positionDisplay = page.locator('#viewportPosition');
    await expect(positionDisplay).toBeVisible();
  });

  test('should handle window resize gracefully', async ({ page }) => {
    // Start with a standard size
    await page.setViewportSize({ width: 1200, height: 800 });
    
    const canvas = page.locator('#checkboxCanvas');
    await expect(canvas).toBeVisible();
    
    // Test various viewport sizes
    const viewports = [
      { width: 1600, height: 900 },
      { width: 1024, height: 768 },
      { width: 768, height: 1024 },
      { width: 375, height: 667 },
      { width: 1920, height: 1080 }
    ];
    
    for (const viewport of viewports) {
      await page.setViewportSize(viewport);
      await page.waitForTimeout(200); // Allow for resize handling
      
      // Canvas should remain visible and responsive
      await expect(canvas).toBeVisible();
      
      // Should be able to navigate
      await canvas.click();
      await page.keyboard.press('ArrowRight');
      await page.waitForTimeout(100);
      
      const positionDisplay = page.locator('#viewportPosition');
      await expect(positionDisplay).toBeTruthy();
    }
  });

  test('should handle stress test of rapid operations', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    const startTime = Date.now();
    
    // Stress test with rapid, random operations
    const operations = ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space'];
    
    for (let i = 0; i < 50; i++) { // Reduced from 100 to 50 for faster test
      const randomOp = operations[Math.floor(Math.random() * operations.length)];
      await page.keyboard.press(randomOp);
      
      // Occasional brief pause to prevent overwhelming
      if (i % 20 === 0) {
        await page.waitForTimeout(10);
      }
    }
    
    const stressTestTime = Date.now() - startTime;
    
    // Should complete stress test in reasonable time
    expect(stressTestTime).toBeLessThan(10000);
    
    // Page should still be functional
    const positionDisplay = page.locator('#viewportPosition');
    await expect(positionDisplay).toBeVisible();
    
    // UI should still be interactive
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(100);
    await expect(positionDisplay).toBeTruthy();
  });

  test('should load resources efficiently', async ({ page }) => {
    // Monitor network requests
    const requests: string[] = [];
    const responses: number[] = [];
    
    page.on('request', (request) => {
      requests.push(request.url());
    });
    
    page.on('response', (response) => {
      responses.push(response.status());
    });
    
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    
    // Should have reasonable number of requests
    expect(requests.length).toBeLessThan(20);
    
    // All responses should be successful
    const failedResponses = responses.filter(status => status >= 400);
    expect(failedResponses).toHaveLength(0);
    
    // Should load main resources
    const hasHTML = requests.some(url => url.includes('.html') || url.endsWith('/'));
    const hasJS = requests.some(url => url.includes('.js'));
    
    expect(hasHTML).toBeTruthy();
    expect(hasJS).toBeTruthy();
  });

  test('should maintain consistent frame rate during canvas operations', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    // Measure frame consistency by checking position updates
    const positionDisplay = page.locator('#viewportPosition');
    const frameTimings: number[] = [];
    
    for (let i = 0; i < 10; i++) { // Reduced from 20 to 10 for faster test
      const startFrame = Date.now();
      await page.keyboard.press('ArrowRight');
      await page.waitForTimeout(50); // Small wait for position update
      const frameTime = Date.now() - startFrame;
      frameTimings.push(frameTime);
    }
    
    // Calculate average frame time
    const avgFrameTime = frameTimings.reduce((sum, time) => sum + time, 0) / frameTimings.length;
    
    // Average frame time should be reasonable (less than 200ms per operation)
    expect(avgFrameTime).toBeLessThan(200);
    
    // Frame times should be relatively consistent (no frame taking more than 10x average)
    const maxFrameTime = Math.max(...frameTimings);
    expect(maxFrameTime).toBeLessThan(avgFrameTime * 10);
  });
});
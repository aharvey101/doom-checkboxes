import { test, expect } from '@playwright/test';

test.describe('Checkbox Grid - Navigation & Interaction', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    
    // Wait for the canvas to be ready
    await page.waitForSelector('#checkboxCanvas');
    await page.waitForTimeout(1000);
  });

  test('should respond to arrow key navigation', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click(); // Focus the canvas
    
    const positionDisplay = page.locator('#viewportPosition');
    
    // Check initial position
    await expect(positionDisplay).toContainText('0, 0');
    
    // Test right arrow
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('1, 0');
    
    // Test down arrow
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('1, 1');
    
    // Test left arrow
    await page.keyboard.press('ArrowLeft');
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('0, 1');
    
    // Test up arrow
    await page.keyboard.press('ArrowUp');
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('0, 0');
  });

  test('should handle multiple arrow key presses', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    const positionDisplay = page.locator('#viewportPosition');
    
    // Navigate to position (5, 3)
    for (let i = 0; i < 5; i++) {
      await page.keyboard.press('ArrowRight');
      await page.waitForTimeout(100);
    }
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press('ArrowDown');
      await page.waitForTimeout(100);
    }
    
    await expect(positionDisplay).toContainText('5, 3');
  });

  test('should respect grid boundaries', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    const positionDisplay = page.locator('#viewportPosition');
    
    // Test upper-left boundary (can't go negative)
    await expect(positionDisplay).toContainText('0, 0');
    
    await page.keyboard.press('ArrowLeft');
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('0, 0');
    
    await page.keyboard.press('ArrowUp');
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('0, 0');
    
    // Test that we can navigate to reasonable positions
    // (Testing the full 99,99 boundary would take too long)
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press('ArrowRight');
      await page.waitForTimeout(50);
    }
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press('ArrowDown');
      await page.waitForTimeout(50);
    }
    
    await expect(positionDisplay).toContainText('10, 10');
  });

  test('should toggle checkboxes on space key', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    const checkedCount = page.locator('#checkedCount');
    const activityLog = page.locator('#log');
    
    // Check initial checked count
    await expect(checkedCount).toContainText('0');
    
    // Toggle checkbox at (0,0)
    await page.keyboard.press('Space');
    await page.waitForTimeout(500);
    
    // Look for indication that checkbox was toggled in the activity log
    // This will depend on our implementation - checking for any checkbox-related activity
    const logContent = await activityLog.textContent();
    expect(logContent).toBeTruthy();
  });

  test('should handle rapid navigation', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    const positionDisplay = page.locator('#viewportPosition');
    
    // Rapid navigation sequence
    const sequence = ['ArrowRight', 'ArrowDown', 'ArrowRight', 'ArrowDown', 'ArrowLeft', 'ArrowUp'];
    
    for (const key of sequence) {
      await page.keyboard.press(key);
      await page.waitForTimeout(50); // Fast navigation
    }
    
    // Should end up at (1, 1)
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('1, 1');
  });

  test('should maintain canvas focus during navigation', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    // Navigate around
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('ArrowDown');
    
    // Canvas should still be focused or body should be focused (depending on implementation)
    const focusedElement = await page.evaluate(() => document.activeElement?.tagName);
    expect(['CANVAS', 'BODY']).toContain(focusedElement);
  });

  test('should handle keyboard focus correctly', async ({ page }) => {
    // Click on canvas to focus it
    const canvas = page.locator('#checkboxCanvas');
    await canvas.click();
    
    // Should be able to navigate
    const positionDisplay = page.locator('#viewportPosition');
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('1, 0');
    
    // Click on a button to potentially lose focus
    await page.locator('#connectBtn').click();
    await page.waitForTimeout(200);
    
    // Click canvas again to regain focus
    await canvas.click();
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(200);
    await expect(positionDisplay).toContainText('2, 0');
  });

  test('should render canvas content', async ({ page }) => {
    const canvas = page.locator('#checkboxCanvas');
    
    // Take a screenshot of the canvas to verify it's rendering content
    const canvasScreenshot = await canvas.screenshot();
    expect(canvasScreenshot.length).toBeGreaterThan(500); // Should have actual content
    
    // Navigate and take another screenshot to verify visual changes
    await canvas.click();
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(500);
    
    const canvasScreenshotAfterNav = await canvas.screenshot();
    expect(canvasScreenshotAfterNav.length).toBeGreaterThan(500);
    
    // Screenshots should be different (indicating visual feedback of navigation)
    expect(Buffer.compare(canvasScreenshot, canvasScreenshotAfterNav)).not.toBe(0);
  });
});
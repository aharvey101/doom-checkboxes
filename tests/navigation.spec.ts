import { test, expect } from '@playwright/test';

test.describe('Chunk Navigation Tests', () => {
  test('click in panned area works', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(2000);

    const canvas = page.locator('canvas');
    const box = await canvas.boundingBox();
    if (!box) throw new Error('Canvas not found');

    // Take screenshot before
    const before = await canvas.screenshot();

    // Click a checkbox
    await canvas.click({ position: { x: 200, y: 200 } });
    await page.waitForTimeout(500);

    // Take screenshot after
    const after = await canvas.screenshot();

    // Screenshots should differ (checkbox toggled)
    expect(Buffer.compare(before, after)).not.toBe(0);
    console.log('Click successfully changed canvas state');
  });

  test('viewport position persists across page refresh', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(2000);

    const canvas = page.locator('canvas');
    await expect(canvas).toBeVisible();

    const box = await canvas.boundingBox();
    if (!box) throw new Error('Canvas not found');

    // Pan to a specific location
    const startX = box.x + box.width / 2;
    const startY = box.y + box.height / 2;

    // Shift+drag to pan significantly
    await page.keyboard.down('Shift');
    await page.mouse.move(startX, startY);
    await page.mouse.down();
    await page.mouse.move(startX - 300, startY - 200, { steps: 10 });
    await page.mouse.up();
    await page.keyboard.up('Shift');

    // Zoom out a bit
    for (let i = 0; i < 5; i++) {
      await page.mouse.wheel(0, 100);
      await page.waitForTimeout(50);
    }

    // Wait for localStorage to save
    await page.waitForTimeout(500);

    // Get the viewport state from localStorage
    const savedViewport = await page.evaluate(() => {
      return localStorage.getItem('checkbox_viewport');
    });

    console.log('Saved viewport:', savedViewport);
    expect(savedViewport).not.toBeNull();

    // Parse the saved values
    const [savedOffsetX, savedOffsetY, savedScale] = savedViewport!.split(',').map(Number);

    // Refresh the page
    await page.reload();
    await page.waitForTimeout(2000);

    // Check that viewport was restored from localStorage
    const restoredViewport = await page.evaluate(() => {
      return localStorage.getItem('checkbox_viewport');
    });

    console.log('Restored viewport:', restoredViewport);
    expect(restoredViewport).toBe(savedViewport);

    // Verify the position is roughly the same (allow some tolerance due to reactive effects)
    const [restoredX, restoredY, restoredScale] = restoredViewport!.split(',').map(Number);
    expect(Math.abs(restoredX - savedOffsetX)).toBeLessThan(1);
    expect(Math.abs(restoredY - savedOffsetY)).toBeLessThan(1);
    expect(Math.abs(restoredScale - savedScale)).toBeLessThan(0.01);
  });

  test('URL bookmark takes priority over localStorage', async ({ page }) => {
    // First, set a viewport in localStorage by visiting the page
    await page.goto('/');
    await page.waitForTimeout(1000);

    // Pan somewhere
    const canvas = page.locator('canvas');
    const box = await canvas.boundingBox();
    if (!box) throw new Error('Canvas not found');

    await page.keyboard.down('Shift');
    await page.mouse.move(box.x + 100, box.y + 100);
    await page.mouse.down();
    await page.mouse.move(box.x + 400, box.y + 300, { steps: 5 });
    await page.mouse.up();
    await page.keyboard.up('Shift');

    await page.waitForTimeout(500);

    // Verify localStorage has a saved viewport
    const savedBefore = await page.evaluate(() => localStorage.getItem('checkbox_viewport'));
    console.log('Saved viewport before URL nav:', savedBefore);
    expect(savedBefore).not.toBeNull();

    // Now navigate with URL bookmark params
    await page.goto('/?x=5000&y=3000&z=0.5');
    await page.waitForTimeout(2000);

    // The URL bookmark should have been applied, and localStorage should now reflect that position
    const savedAfter = await page.evaluate(() => localStorage.getItem('checkbox_viewport'));
    console.log('Saved viewport after URL nav:', savedAfter);

    // The saved viewport should be different (URL bookmark was applied)
    expect(savedAfter).not.toBe(savedBefore);
  });
});

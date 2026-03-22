import { test, expect } from '@playwright/test';
import * as fs from 'fs';

test.describe('Drag to Fill', () => {
  test('dragging fills multiple checkboxes', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(2000);

    const canvas = page.locator('canvas');
    await expect(canvas).toBeVisible();

    const box = await canvas.boundingBox();
    if (!box) throw new Error('Canvas not found');

    // Pan to a random fresh area that's unlikely to have existing checkboxes
    // Use a random offset based on timestamp to get unique positions each run
    const randomOffset = Date.now() % 10000;
    await page.keyboard.down('Shift');
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    // Pan right and down by a large random amount
    await page.mouse.move(
      box.x + box.width / 2 - 500 - (randomOffset % 500), 
      box.y + box.height / 2 - 500 - (randomOffset % 300), 
      { steps: 10 }
    );
    await page.mouse.up();
    await page.keyboard.up('Shift');
    await page.waitForTimeout(500);

    // Take screenshot before
    const before = await canvas.screenshot();
    fs.writeFileSync('drag-before.png', before);

    // Drag across multiple cells (without shift key)
    // Draw in the center of the canvas where the grid should be visible
    const startX = box.x + box.width / 2;
    const startY = box.y + box.height / 2;
    const endX = startX + 200;
    const endY = startY;

    await page.mouse.move(startX, startY);
    await page.mouse.down();
    
    // Move slowly across cells
    for (let x = startX; x <= endX; x += 10) {
      await page.mouse.move(x, startY);
      await page.waitForTimeout(10);
    }
    
    await page.mouse.up();
    await page.waitForTimeout(500);

    // Take screenshot after
    const after = await canvas.screenshot();
    fs.writeFileSync('drag-after.png', after);

    // Screenshots should differ (checkboxes were filled)
    const changed = Buffer.compare(before, after) !== 0;
    console.log('Drag changed canvas:', changed);
    expect(changed).toBe(true);
  });

  test('drag creates line of filled checkboxes', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(2000);

    const canvas = page.locator('canvas');
    const box = await canvas.boundingBox();
    if (!box) throw new Error('Canvas not found');

    // Draw a diagonal line
    const startX = box.x + 150;
    const startY = box.y + 150;
    const endX = box.x + 350;
    const endY = box.y + 250;

    await page.mouse.move(startX, startY);
    await page.mouse.down();
    
    // Move diagonally
    const steps = 20;
    for (let i = 0; i <= steps; i++) {
      const x = startX + (endX - startX) * (i / steps);
      const y = startY + (endY - startY) * (i / steps);
      await page.mouse.move(x, y);
      await page.waitForTimeout(10);
    }
    
    await page.mouse.up();
    await page.waitForTimeout(500);

    // Take screenshot
    const screenshot = await canvas.screenshot();
    fs.writeFileSync('drag-diagonal.png', screenshot);

    console.log('Diagonal drag completed');
  });
});

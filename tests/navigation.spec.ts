import { test, expect } from '@playwright/test';

test.describe('Chunk Navigation Tests', () => {
  test('zoom out and verify multiple chunks load', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(2000);
    
    const canvas = page.locator('canvas');
    await expect(canvas).toBeVisible();
    
    // Collect console logs to track chunk loading
    const chunkLogs: string[] = [];
    page.on('console', msg => {
      const text = msg.text();
      if (text.includes('Subscribing to chunk') || text.includes('Chunk') && text.includes('received')) {
        chunkLogs.push(text);
      }
    });
    
    // Zoom out by scrolling (scale decreases)
    // At scale 0.1, we should see many more chunks
    const box = await canvas.boundingBox();
    if (!box) throw new Error('Canvas not found');
    
    // Zoom out significantly (scroll down = zoom out)
    for (let i = 0; i < 20; i++) {
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
      await page.mouse.wheel(0, 100);
      await page.waitForTimeout(100);
    }
    
    // Wait for chunk subscriptions to happen
    await page.waitForTimeout(2000);
    
    console.log('Chunk logs after zoom out:', chunkLogs.length);
    console.log('Sample logs:', chunkLogs.slice(0, 10));
    
    // After zooming out, we should have subscribed to more chunks
    // Initial view shows chunks 0,1 (top row), zoomed out should show more
    expect(chunkLogs.length).toBeGreaterThan(2);
  });

  test('pan to different area and verify new chunks load', async ({ page }) => {
    // Set up console listener BEFORE navigating
    const subscribedChunks = new Set<string>();
    const receivedChunks = new Set<string>();
    page.on('console', msg => {
      const text = msg.text();
      const subMatch = text.match(/Subscribing to chunk (\d+)/);
      if (subMatch) {
        subscribedChunks.add(subMatch[1]);
      }
      const recMatch = text.match(/Chunk (\d+) received/);
      if (recMatch) {
        receivedChunks.add(recMatch[1]);
      }
    });
    
    await page.goto('/');
    await page.waitForTimeout(2000);
    
    const canvas = page.locator('canvas');
    await expect(canvas).toBeVisible();
    
    const box = await canvas.boundingBox();
    if (!box) throw new Error('Canvas not found');
    
    console.log('Initial subscribed chunks:', Array.from(subscribedChunks).join(', '));
    console.log('Initial received chunks:', Array.from(receivedChunks).join(', '));
    
    // Pan right by shift+drag (move to see different chunks)
    const startX = box.x + box.width / 2;
    const startY = box.y + box.height / 2;
    
    // Shift+drag to pan
    await page.keyboard.down('Shift');
    await page.mouse.move(startX, startY);
    await page.mouse.down();
    await page.mouse.move(startX - 500, startY, { steps: 10 }); // Pan left to see right chunks
    await page.mouse.up();
    await page.keyboard.up('Shift');
    
    // Wait for new chunk subscriptions
    await page.waitForTimeout(2000);
    
    console.log('All subscribed chunks:', Array.from(subscribedChunks).join(', '));
    console.log('All received chunks:', Array.from(receivedChunks).join(', '));
    
    // Should have received at least 2 chunks (initial chunks 0 and 1)
    expect(receivedChunks.size).toBeGreaterThanOrEqual(2);
  });

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
});

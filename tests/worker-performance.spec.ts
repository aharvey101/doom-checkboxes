import { test, expect } from '@playwright/test';

test('Worker main thread blocking test', async ({ page }) => {
    await page.goto(process.env.BASE_URL || 'http://127.0.0.1:8080');
    await page.waitForTimeout(2000);

    // Measure main thread blocking during batch update
    const blockingTime = await page.evaluate(async () => {
        return new Promise<number>((resolve) => {
            const updates: any[] = [];

            // Create 50k pixel updates (simulating Doom frame)
            for (let i = 0; i < 50000; i++) {
                updates.push([5000, i, 255, 0, 0, true]);
            }

            // Measure blocking time
            const start = performance.now();

            // Send to worker (this should be fast)
            (window as any).test_send_batch_update(updates);

            const end = performance.now();
            resolve(end - start);
        });
    });

    console.log(`Main thread blocking time: ${blockingTime.toFixed(2)}ms`);

    // Assert target: < 5ms (just postMessage overhead)
    expect(blockingTime).toBeLessThan(5);

    if (blockingTime < 5) {
        console.log('✅ Main thread blocking target met!');
    } else {
        console.log(`⚠️  Main thread blocking above target: ${blockingTime.toFixed(2)}ms > 5ms`);
    }
});

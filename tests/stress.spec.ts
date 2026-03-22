import { test, expect } from "@playwright/test";

test.describe("Checkbox Grid Stress Test", () => {
  test("measure actual in-browser click responsiveness", async ({ page }) => {
    await page.goto("/");

    // Wait for the app to load (Leptos/WASM app)
    await page.waitForSelector("canvas", { timeout: 15000 });

    // Give WASM time to initialize
    await page.waitForTimeout(2000);

    console.log("App loaded, measuring click responsiveness in-browser");

    // Inject performance measurement directly into the page
    const results = await page.evaluate(async () => {
      const canvas = document.querySelector("canvas") as HTMLCanvasElement;
      if (!canvas) return { error: "No canvas found" };

      const clickTimes: number[] = [];
      const NUM_CLICKS = 50;

      for (let i = 0; i < NUM_CLICKS; i++) {
        const x = 50 + (i % 10) * 30;
        const y = 50 + Math.floor(i / 10) * 30;

        const start = performance.now();

        // Dispatch a real click event
        const rect = canvas.getBoundingClientRect();
        const clickEvent = new MouseEvent("click", {
          clientX: rect.left + x,
          clientY: rect.top + y,
          bubbles: true,
        });
        canvas.dispatchEvent(clickEvent);

        // Small delay to let the UI update
        await new Promise(r => setTimeout(r, 10));

        const elapsed = performance.now() - start;
        clickTimes.push(elapsed);
      }

      return {
        clickTimes,
        avg: clickTimes.reduce((a, b) => a + b, 0) / clickTimes.length,
        max: Math.max(...clickTimes),
        min: Math.min(...clickTimes),
        count: NUM_CLICKS,
      };
    });

    if ('error' in results) {
      console.log("Error:", results.error);
      throw new Error(results.error);
    }

    console.log("\n" + "=".repeat(60));
    console.log("IN-BROWSER CLICK RESPONSIVENESS");
    console.log("=".repeat(60));
    console.log(`Clicks measured: ${results.count}`);
    console.log(`Avg time: ${results.avg.toFixed(2)}ms`);
    console.log(`Min: ${results.min.toFixed(2)}ms`);
    console.log(`Max: ${results.max.toFixed(2)}ms`);

    expect(results.avg).toBeLessThan(100);
  });

  test("rapid clicking stress test", async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector("canvas", { timeout: 15000 });
    await page.waitForTimeout(2000);

    const canvas = page.locator("canvas");

    console.log("Starting rapid click test...");

    const NUM_CLICKS = 100;
    const startTime = Date.now();

    for (let i = 0; i < NUM_CLICKS; i++) {
      await canvas.click({
        position: {
          x: 50 + Math.random() * 400,
          y: 50 + Math.random() * 300,
        },
        force: true,
      });
    }

    const elapsed = Date.now() - startTime;
    console.log(`\n${NUM_CLICKS} clicks in ${elapsed}ms`);
    console.log(`Clicks per second: ${((NUM_CLICKS / elapsed) * 1000).toFixed(1)}`);
  });
});

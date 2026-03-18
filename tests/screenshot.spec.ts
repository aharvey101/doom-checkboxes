import { test } from "@playwright/test";

test("capture app screenshot", async ({ page }) => {
  await page.goto("/");
  
  // Wait for canvas and WASM to load
  await page.waitForSelector("canvas", { timeout: 15000 });
  await page.waitForTimeout(3000);
  
  // Take full page screenshot
  await page.screenshot({ path: "app-screenshot.png", fullPage: true });
  
  console.log("Screenshot saved to app-screenshot.png");
});

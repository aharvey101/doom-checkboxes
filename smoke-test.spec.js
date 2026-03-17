import { test, expect } from '@playwright/test';

test('basic smoke test - page loads', async ({ page }) => {
  // Set a short timeout for this test
  test.setTimeout(30000);
  
  console.log('Starting smoke test...');
  
  // Navigate to the page
  await page.goto('/');
  
  // Wait for the page to load
  await page.waitForLoadState('networkidle');
  
  // Check that the page has loaded
  const title = await page.title();
  console.log('Page title:', title);
  
  // Check for basic elements
  const body = await page.locator('body');
  await expect(body).toBeVisible();
  
  console.log('✅ Smoke test passed');
});

test('vite dev server responding', async ({ page }) => {
  test.setTimeout(15000);
  
  // Just check if we can connect
  const response = await page.goto('/');
  expect(response.status()).toBe(200);
  
  console.log('✅ Vite dev server is responding');
});
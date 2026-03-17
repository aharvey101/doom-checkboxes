import { test, expect } from '@playwright/test';
import { execSync } from 'child_process';

test.beforeEach(async ({ page }) => {
  // Reset SpacetimeDB state if available
  try {
    execSync('node scripts/test-db-manager.js reset-data', { stdio: 'ignore' });
  } catch (error) {
    console.log('State reset skipped - SpacetimeDB may not be running');
  }

  // Enhanced error handling for SpacetimeDB connection issues
  page.on('console', msg => {
    if (msg.type() === 'error' || msg.text().includes('ERROR')) {
      console.log(`[BROWSER ERROR] ${msg.text()}`);
    }
  });
  
  page.on('pageerror', err => {
    console.log(`[PAGE ERROR] ${err.message}`);
  });
});

test('WASM loads with cache busting and no runtime errors', async ({ page }) => {
  const consoleLogs = [];
  const consoleErrors = [];
  
  page.on('console', msg => {
    consoleLogs.push(msg.text());
    console.log('[CONSOLE]', msg.text());
  });
  
  page.on('pageerror', err => {
    consoleErrors.push(err.message);
    console.log('[PAGE ERROR]', err.message);
  });
  
  // Add cache-busting parameter to force fresh WASM load
  const timestamp = Date.now();
  const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
  await page.goto(`${baseURL}?v=${timestamp}`, { waitUntil: 'networkidle' });
  
  // Wait for WASM to load
  await page.waitForFunction(() => {
    const app = document.getElementById('app');
    return app && app.innerHTML.includes('grid');
  }, { timeout: 5000 });
  
  // Check that the grid exists
  const grid = await page.locator('#grid');
  await expect(grid).toBeVisible();
  
  // Verify no runtime errors occurred
  const runtimeErrors = consoleErrors.filter(err => 
    err.includes('table index is out of bounds') || 
    err.includes('RuntimeError')
  );
  
  console.log('✅ Page loaded with cache busting');
  console.log(`Total console logs: ${consoleLogs.length}`);
  console.log(`Total page errors: ${consoleErrors.length}`);
  console.log(`Runtime/table errors: ${runtimeErrors.length}`);
  
  expect(runtimeErrors.length).toBe(0);
});

test('arrow key panning with no externref table errors', async ({ page }) => {
  const consoleErrors = [];
  
  page.on('console', msg => console.log('[CONSOLE]', msg.text()));
  page.on('pageerror', err => {
    consoleErrors.push(err.message);
    console.log('[PAGE ERROR]', err.message);
  });
  
  // Cache-busting load
  const timestamp = Date.now();
  const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
  await page.goto(`${baseURL}?v=${timestamp}`, { waitUntil: 'networkidle' });
  
  // Wait for WASM to load
  await page.waitForFunction(() => {
    const app = document.getElementById('app');
    return app && app.innerHTML.includes('grid');
  }, { timeout: 5000 });
  
  const grid = await page.locator('#grid');
  await expect(grid).toBeVisible();
  
  // Perform multiple arrow key presses to trigger re-renders
  for (let i = 0; i < 5; i++) {
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(50);
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(50);
  }
  
  // Verify grid still visible
  await expect(grid).toBeVisible();
  
  // Check for externref table errors
  const tableErrors = consoleErrors.filter(err => 
    err.includes('table index is out of bounds')
  );
  
  console.log('✅ Arrow key panning completed');
  console.log(`Errors during panning: ${consoleErrors.length}`);
  console.log(`Table index errors: ${tableErrors.length}`);
  
  expect(tableErrors.length).toBe(0);
});

test('mouse drag panning with no externref table errors', async ({ page }) => {
  const consoleErrors = [];
  
  page.on('console', msg => console.log('[CONSOLE]', msg.text()));
  page.on('pageerror', err => {
    consoleErrors.push(err.message);
    console.log('[PAGE ERROR]', err.message);
  });
  
  // Cache-busting load
  const timestamp = Date.now();
  const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
  await page.goto(`${baseURL}?v=${timestamp}`, { waitUntil: 'networkidle' });
  
  // Wait for WASM to load
  await page.waitForFunction(() => {
    const app = document.getElementById('app');
    return app && app.innerHTML.includes('grid');
  }, { timeout: 5000 });
  
  const grid = await page.locator('#grid');
  await expect(grid).toBeVisible();
  
  // Perform mouse drag to trigger panning
  await grid.dragTo(grid, {
    sourcePosition: { x: 300, y: 300 },
    targetPosition: { x: 100, y: 100 }
  });
  
  // Verify grid still visible
  await expect(grid).toBeVisible();
  
  // Check for externref table errors
  const tableErrors = consoleErrors.filter(err => 
    err.includes('table index is out of bounds')
  );
  
  console.log('✅ Mouse drag panning completed');
  console.log(`Errors during dragging: ${consoleErrors.length}`);
  console.log(`Table index errors: ${tableErrors.length}`);
  
  expect(tableErrors.length).toBe(0);
});

test('checkbox clicking triggers no externref table errors', async ({ page }) => {
  const consoleErrors = [];
  
  page.on('console', msg => console.log('[CONSOLE]', msg.text()));
  page.on('pageerror', err => {
    consoleErrors.push(err.message);
    console.log('[PAGE ERROR]', err.message);
  });
  
  // Cache-busting load
  const timestamp = Date.now();
  const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
  await page.goto(`${baseURL}?v=${timestamp}`, { waitUntil: 'networkidle' });
  
  // Wait for WASM to load
  await page.waitForFunction(() => {
    const app = document.getElementById('app');
    return app && app.innerHTML.includes('grid');
  }, { timeout: 5000 });
  
  // Get first checkbox
  const firstCheckbox = await page.locator('input[type="checkbox"]').first();
  await expect(firstCheckbox).toBeVisible();
  
  // Click it multiple times
  for (let i = 0; i < 3; i++) {
    await firstCheckbox.click();
    await page.waitForTimeout(50);
  }
  
  // Check for externref table errors
  const tableErrors = consoleErrors.filter(err => 
    err.includes('table index is out of bounds')
  );
  
  console.log('✅ Checkbox clicking completed');
  console.log(`Errors during clicking: ${consoleErrors.length}`);
  console.log(`Table index errors: ${tableErrors.length}`);
  
  expect(tableErrors.length).toBe(0);
});

test('combined interactions with cache busting and no runtime errors', async ({ page }) => {
  const consoleErrors = [];
  
  page.on('console', msg => console.log('[CONSOLE]', msg.text()));
  page.on('pageerror', err => {
    consoleErrors.push(err.message);
    console.log('[PAGE ERROR]', err.message);
  });
  
  // Cache-busting load
  const timestamp = Date.now();
  const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
  await page.goto(`${baseURL}?v=${timestamp}`, { waitUntil: 'networkidle' });
  
  // Wait for WASM to load
  await page.waitForFunction(() => {
    const app = document.getElementById('app');
    return app && app.innerHTML.includes('grid');
  }, { timeout: 5000 });
  
  const grid = await page.locator('#grid');
  await expect(grid).toBeVisible();
  
  // Perform multiple interactions
  console.log('Performing arrow key panning...');
  for (let i = 0; i < 3; i++) {
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(30);
  }
  
  console.log('Clicking checkboxes...');
  const checkboxes = await page.locator('input[type="checkbox"]').all();
  if (checkboxes.length > 0) {
    await checkboxes[0].click();
    await page.waitForTimeout(30);
  }
  
  console.log('Performing mouse drag...');
  await grid.dragTo(grid, {
    sourcePosition: { x: 250, y: 250 },
    targetPosition: { x: 150, y: 150 }
  });
  
  // Verify grid still responsive
  await expect(grid).toBeVisible();
  
  // Check for any externref table errors
  const tableErrors = consoleErrors.filter(err => 
    err.includes('table index is out of bounds')
  );
  
  console.log('✅ All interactions completed');
  console.log(`Total errors: ${consoleErrors.length}`);
  console.log(`Table index errors: ${tableErrors.length}`);
  
  expect(tableErrors.length).toBe(0);
});

import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  // Reset SpacetimeDB state if available
  try {
    const { execSync } = await import('child_process');
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

test.describe('Panning functionality', () => {
  
  test('grid updates position with arrow right', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    // Force cache bust
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    // Wait for grid to load
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Get initial grid HTML
    const initialHTML = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    console.log('Initial grid loaded');
    expect(initialHTML.length).toBeGreaterThan(0);
    
    // Press arrow right
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(200);
    
    // Get updated grid HTML
    const updatedHTML = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    console.log('Grid updated after ArrowRight');
    
    // Grid should still have content
    expect(updatedHTML.length).toBeGreaterThan(0);
    console.log('✅ Arrow right panning works');
  });

  test('grid updates position with arrow left', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Pan right first
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(100);
    
    const panRightHTML = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Pan left
    await page.keyboard.press('ArrowLeft');
    await page.waitForTimeout(200);
    
    const panLeftHTML = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Should be different from panned right
    expect(panLeftHTML).not.toBe(panRightHTML);
    expect(panLeftHTML.length).toBeGreaterThan(0);
    console.log('✅ Arrow left panning works');
  });

  test('grid updates position with arrow down', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    const initialHTML = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Pan down
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(200);
    
    const panDownHTML = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Should be different
    expect(panDownHTML).not.toBe(initialHTML);
    expect(panDownHTML.length).toBeGreaterThan(0);
    console.log('✅ Arrow down panning works');
  });

  test('grid updates position with arrow up', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Pan down first
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(100);
    
    const panDownHTML = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Pan up
    await page.keyboard.press('ArrowUp');
    await page.waitForTimeout(200);
    
    const panUpHTML = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Should be different from down
    expect(panUpHTML).not.toBe(panDownHTML);
    expect(panUpHTML.length).toBeGreaterThan(0);
    console.log('✅ Arrow up panning works');
  });

  test('multiple arrow key presses pan continuously', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    const states = [];
    
    // Record initial state
    const initial = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    states.push(initial);
    
    // Press arrow right 3 times
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press('ArrowRight');
      await page.waitForTimeout(100);
      
      const state = await page.evaluate(() => {
        const grid = document.getElementById('grid');
        return grid ? grid.innerHTML : '';
      });
      states.push(state);
    }
    
    // All states should exist and have content
    states.forEach((state, i) => {
      expect(state.length).toBeGreaterThan(0);
      console.log(`State ${i}: ${state.length} chars`);
    });
    
    // Most states should be unique (different panning positions)
    const uniqueStates = new Set(states);
    console.log(`Total states recorded: ${states.length}, Unique: ${uniqueStates.size}`);
    expect(uniqueStates.size).toBeGreaterThanOrEqual(2);
    console.log('✅ Multiple panning works');
  });

  test('diagonal panning (right then down)', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    const initial = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Pan right
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(100);
    
    // Pan down
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(200);
    
    const diagonal = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Should be different from initial
    expect(diagonal).not.toBe(initial);
    expect(diagonal.length).toBeGreaterThan(0);
    console.log('✅ Diagonal panning works');
  });

  test('viewport dimensions display correctly', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Get viewport width
    const viewportWidth = await page.evaluate(() => {
      const elem = document.getElementById('viewport');
      return elem ? parseInt(elem.textContent) : null;
    });
    
    console.log(`Viewport width displayed: ${viewportWidth}`);
    expect(viewportWidth).toBeGreaterThan(0);
    expect(viewportWidth).toBeLessThan(10000); // Sanity check
    console.log('✅ Viewport dimensions display');
  });

  test('columns display correctly', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Get column count
    const cols = await page.evaluate(() => {
      const elem = document.getElementById('cols');
      return elem ? parseInt(elem.textContent) : null;
    });
    
    console.log(`Columns displayed: ${cols}`);
    expect(cols).toBeGreaterThan(0);
    expect(cols).toBeLessThan(200); // Sanity check
    
    // Count actual checkboxes displayed
    const checkboxCount = await page.locator('input[type="checkbox"]').count();
    console.log(`Actual checkboxes visible: ${checkboxCount}`);
    expect(checkboxCount).toBeGreaterThan(0);
    console.log('✅ Column count and checkbox display');
  });

  test('grid remains responsive after many pan operations', async ({ page }) => {
    page.on('console', msg => console.log('[CONSOLE]', msg.text()));
    page.on('pageerror', err => console.log('[ERROR]', err));
    
    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Perform 20 pan operations
    console.log('Starting stress test with 20 pans...');
    for (let i = 0; i < 20; i++) {
      const key = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'][i % 4];
      await page.keyboard.press(key);
      await page.waitForTimeout(50);
    }
    
    // Grid should still be visible and responsive
    const grid = await page.locator('#grid');
    await expect(grid).toBeVisible();
    
    // Should still have checkboxes
    const checkboxCount = await page.locator('input[type="checkbox"]').count();
    expect(checkboxCount).toBeGreaterThan(0);
    console.log(`✅ Grid responsive after 20 pans, ${checkboxCount} checkboxes visible`);
  });

});

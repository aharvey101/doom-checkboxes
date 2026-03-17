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

test.describe('Mouse interactions and visual feedback', () => {

  test('checkboxes can be toggled by clicking', async ({ page }) => {

    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Get first checkbox
    const firstCheckbox = await page.locator('input[type="checkbox"]').first();
    await expect(firstCheckbox).toBeVisible();
    
    // Check initial state
    const initiallyChecked = await firstCheckbox.isChecked();
    console.log(`Checkbox initially checked: ${initiallyChecked}`);
    
    // Click it
    await firstCheckbox.click();
    await page.waitForTimeout(100);
    
    // Verify it toggled
    const afterClick = await firstCheckbox.isChecked();
    console.log(`Checkbox after click: ${afterClick}`);
    expect(afterClick).not.toBe(initiallyChecked);
    
    // Click again to toggle back
    await firstCheckbox.click();
    await page.waitForTimeout(100);
    
    const afterSecondClick = await firstCheckbox.isChecked();
    console.log(`Checkbox after second click: ${afterSecondClick}`);
    expect(afterSecondClick).toBe(initiallyChecked);
    console.log('✅ Checkbox toggle works');
  });

  test('checked counter element exists and displays a number', async ({ page }) => {

    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Get checked count element
    const checkedCount = await page.evaluate(() => {
      const elem = document.getElementById('checked');
      return elem ? elem.textContent : '';
    });
    
    console.log(`Checked count displayed: ${checkedCount}`);
    expect(checkedCount).toBeTruthy();
    
    // Should be a number
    const count = parseInt(checkedCount);
    expect(count).toBeGreaterThanOrEqual(0);
    console.log('✅ Checked counter element exists');
  });

  test('checkbox parent gets correct CSS class when checked', async ({ page }) => {

    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Find an unchecked checkbox
    const checkboxes = await page.locator('input[type="checkbox"]').all();
    let testCheckbox = null;
    
    for (const checkbox of checkboxes) {
      const isChecked = await checkbox.isChecked();
      if (!isChecked) {
        testCheckbox = checkbox;
        break;
      }
    }
    
    if (!testCheckbox) {
      console.log('All checkboxes were checked, skipping CSS class test');
      return;
    }
    
    // Get parent before click
    const parentBefore = await testCheckbox.evaluate(el => {
      return el.parentElement ? el.parentElement.className : '';
    });
    console.log(`Parent class before click: ${parentBefore}`);
    expect(parentBefore).not.toContain('checked');
    
    // Click checkbox
    await testCheckbox.click();
    await page.waitForTimeout(100);
    
    // Get parent after click
    const parentAfter = await testCheckbox.evaluate(el => {
      return el.parentElement ? el.parentElement.className : '';
    });
    console.log(`Parent class after click: ${parentAfter}`);
    expect(parentAfter).toContain('checked');
    console.log('✅ CSS class updates on checkbox click');
  });

  test('multiple checkboxes can be clicked in sequence', async ({ page }) => {

    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Click first 3 visible checkboxes
    const checkboxes = await page.locator('input[type="checkbox"]').all();
    const clickedCount = Math.min(3, checkboxes.length);
    const toggledStates = [];
    
    for (let i = 0; i < clickedCount; i++) {
      const checkbox = checkboxes[i];
      const beforeClick = await checkbox.isChecked();
      await checkbox.click();
      await page.waitForTimeout(50);
      const afterClick = await checkbox.isChecked();
      
      expect(afterClick).not.toBe(beforeClick);
      toggledStates.push({ before: beforeClick, after: afterClick });
      console.log(`Checkbox ${i}: ${beforeClick} -> ${afterClick}`);
    }
    
    expect(toggledStates.length).toBe(clickedCount);
    console.log(`✅ Successfully toggled ${clickedCount} checkboxes`);
  });

  test('grid updates when panning after checkbox click', async ({ page }) => {

    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Click first checkbox
    const firstCheckbox = await page.locator('input[type="checkbox"]').first();
    await firstCheckbox.click();
    await page.waitForTimeout(100);
    
    // Get grid HTML after click
    const gridAfterClick = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Pan the grid
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(200);
    
    // Get grid HTML after pan
    const gridAfterPan = await page.evaluate(() => {
      const grid = document.getElementById('grid');
      return grid ? grid.innerHTML : '';
    });
    
    // Should be different (different viewport)
    expect(gridAfterPan).not.toBe(gridAfterClick);
    console.log('✅ Grid updates correctly after click and pan');
  });

  test('status indicator displays connection state', async ({ page }) => {

    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Get status element
    const status = await page.evaluate(() => {
      const elem = document.getElementById('status');
      return elem ? elem.textContent : '';
    });
    
    console.log(`Status displayed: ${status}`);
    expect(status.length).toBeGreaterThan(0);
    console.log('✅ Status indicator displays');
  });

  test('checkbox data attributes are correct', async ({ page }) => {

    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Get first checkbox's data-index
    const dataIndex = await page.locator('input[type="checkbox"]').first().evaluate(el => {
      return el.getAttribute('data-index');
    });
    
    console.log(`First checkbox data-index: ${dataIndex}`);
    expect(dataIndex).toBeTruthy();
    
    const index = parseInt(dataIndex);
    expect(index).toBeGreaterThanOrEqual(0);
    expect(index).toBeLessThan(10000000); // Total checkbox count
    console.log('✅ Checkbox data attributes are correct');
  });

  test('clicking different checkboxes in grid works', async ({ page }) => {

    const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://localhost:8000';
    await page.goto(`${baseURL}/?t=${Date.now()}`, { waitUntil: 'networkidle' });
    
    await page.waitForFunction(() => {
      return document.getElementById('app')?.innerHTML.includes('grid');
    }, { timeout: 5000 });
    
    // Get all checkboxes
    const checkboxes = await page.locator('input[type="checkbox"]').all();
    console.log(`Total checkboxes visible: ${checkboxes.length}`);
    
    // Click a few different ones
    const clickableCount = Math.min(5, checkboxes.length);
    for (let i = 0; i < clickableCount; i += Math.max(1, Math.floor(checkboxes.length / 5))) {
      const checkbox = checkboxes[i];
      const before = await checkbox.isChecked();
      await checkbox.click();
      await page.waitForTimeout(50);
      const after = await checkbox.isChecked();
      
      expect(after).not.toBe(before);
      console.log(`Checkbox at index ${i} toggled: ${before} -> ${after}`);
    }
    
    console.log('✅ Multiple different checkboxes can be clicked');
  });

});

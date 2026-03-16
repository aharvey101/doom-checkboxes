import { test, expect } from '@playwright/test';

test('checkboxes persist correctly after page refresh', async ({ page }) => {
  // Navigate to the app
  await page.goto('http://localhost:5174/');

  // Initialize console message tracking
  let consoleMessages: any[] = [];
  page.on('console', (msg) => {
    consoleMessages.push({
      type: msg.type(),
      text: msg.text()
    });
  });

  // Wait for the app to initialize and connect
  await page.waitForTimeout(4000);

  // Wait for at least one render call to ensure connection is complete
  let initialRenderCount = 0;
  let retries = 10;
  while (initialRenderCount === 0 && retries > 0) {
    initialRenderCount = consoleMessages.filter(msg => 
      msg.text.includes('🎨 [RENDER] Complete')
    ).length;
    if (initialRenderCount === 0) {
      await page.waitForTimeout(500);
      retries--;
    }
  }
  
  console.log(`Initial render calls detected: ${initialRenderCount}`);

  console.log('=== PAGE REFRESH PERSISTENCE TEST ===');

  const canvas = page.locator('canvas');

  // Click multiple checkboxes in different positions to create a pattern
  console.log('Checking multiple boxes before refresh...');
  
  // Click checkbox at (0, 0) 
  await canvas.click({ position: { x: 16, y: 16 } });
  await page.waitForTimeout(300);
  
  // Click checkbox at (2, 0)  
  await canvas.click({ position: { x: 80, y: 16 } });
  await page.waitForTimeout(300);
  
  // Click checkbox at (0, 2)  
  await canvas.click({ position: { x: 16, y: 80 } });
  await page.waitForTimeout(300);
  
  // Click checkbox at (3, 3) 
  await canvas.click({ position: { x: 112, y: 112 } });
  await page.waitForTimeout(300);

  // Verify all checkboxes are checked before refresh
  const renderMessages = consoleMessages.filter(msg => msg.text.includes('🎨 [RENDER] Complete'));
  const lastRenderMsg = renderMessages[renderMessages.length - 1];
  
  const preRefreshMatch = lastRenderMsg?.text.match(/Checked: (\d+)/);
  const preRefreshState = preRefreshMatch ? parseInt(preRefreshMatch[1]) : 0;

  console.log(`Pre-refresh checked count: ${preRefreshState}`);
  expect(preRefreshState).toBe(4); // Should have 4 checkboxes checked

  // Wait a bit more to ensure database sync completes
  await page.waitForTimeout(1000);

  console.log('Refreshing the page...');

  // Reset console tracking for post-refresh
  consoleMessages = [];
  
  // Refresh the page
  await page.reload();

  console.log('Waiting for auto-connection after refresh...');

  // Wait for auto-connection to complete after refresh
  await page.waitForTimeout(4000);
  
  // Wait for render calls to detect connection completion
  let postRefreshRenderCount = 0;
  let postRetries = 15; // Give more time for refresh
  while (postRefreshRenderCount === 0 && postRetries > 0) {
    postRefreshRenderCount = consoleMessages.filter(msg => 
      msg.text.includes('🎨 [RENDER] Complete')
    ).length;
    if (postRefreshRenderCount === 0) {
      await page.waitForTimeout(1000);
      postRetries--;
    }
  }

  console.log(`Post-refresh render calls detected: ${postRefreshRenderCount}`);

  // Debug: Show all console messages to understand what's happening
  console.log('=== POST-REFRESH CONSOLE MESSAGES ===');
  consoleMessages.forEach((msg, idx) => {
    if (msg.text.includes('chunks') || msg.text.includes('RENDER') || msg.text.includes('DB')) {
      console.log(`${idx}: ${msg.text}`);
    }
  });

  // Get the final checked count after reload
  const postRenderMessages = consoleMessages.filter(msg => msg.text.includes('🎨 [RENDER] Complete'));
  const lastPostRenderMsg = postRenderMessages[postRenderMessages.length - 1];
  
  console.log(`Last render message: ${lastPostRenderMsg?.text || 'none'}`);
  
  const postRefreshMatch = lastPostRenderMsg?.text.match(/Checked: (\d+)/);
  const postRefreshState = postRefreshMatch ? parseInt(postRefreshMatch[1]) : 0;

  console.log(`Post-refresh checked count: ${postRefreshState}`);

  // Verify that all checkboxes remain checked after page refresh
  expect(postRefreshState).toBe(4);
  
  console.log('✅ Page refresh persistence test passed - all checkboxes restored from database');

  // Take screenshot to verify visual state
  await page.screenshot({ path: 'page-refresh-persistence-test.png' });
});
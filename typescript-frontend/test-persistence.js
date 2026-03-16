import { chromium } from 'playwright';

(async () => {
  console.log('🔄 Testing checkbox persistence across page reloads...');
  
  const browser = await chromium.launch({ headless: false });
  const page = await browser.newPage();
  
  page.on('console', msg => console.log(`CONSOLE ${msg.type()}: ${msg.text()}`));
  page.on('pageerror', err => console.log(`PAGE ERROR: ${err.message}`));
  
  // Navigate to the app
  console.log('Opening app...');
  await page.goto('https://checkbox-grid-100x100.netlify.app');
  await page.waitForTimeout(3000);
  
  // Connect to SpacetimeDB
  console.log('Connecting to SpacetimeDB...');
  await page.click('#connectBtn');
  await page.waitForTimeout(8000);
  
  // Click a specific checkbox to create a unique pattern
  console.log('Creating unique test pattern...');
  const testClicks = [
    { x: 50, y: 50 },   // (1, 1)  
    { x: 150, y: 50 },  // (1, 3)
    { x: 50, y: 150 },  // (3, 1) 
    { x: 150, y: 150 }  // (3, 3)
  ];
  
  for (const click of testClicks) {
    await page.click('canvas', { position: click });
    await page.waitForTimeout(1000); // Wait for sync
  }
  
  console.log('Test pattern created. Now reloading page...');
  
  // Reload the page 
  await page.reload();
  await page.waitForTimeout(3000);
  
  // Reconnect to SpacetimeDB
  console.log('Reconnecting after reload...');
  await page.click('#connectBtn');
  await page.waitForTimeout(8000);
  
  console.log('✅ Persistence test complete!');
  console.log('📊 Check the console logs to verify the checkboxes were loaded from SpacetimeDB after reload.');
  console.log('🔍 Look for messages like "📦 Loaded X chunks from SpacetimeDB" with existing data.');
  
  await page.waitForTimeout(5000); // Give time to see the result
  await browser.close();
})();
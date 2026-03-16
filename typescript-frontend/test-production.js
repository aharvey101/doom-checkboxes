import { chromium } from 'playwright';

(async () => {
  const browser = await chromium.launch({ headless: false });
  const page = await browser.newPage();
  
  // Listen for console messages
  page.on('console', msg => {
    console.log(`CONSOLE ${msg.type()}: ${msg.text()}`);
  });
  
  // Listen for errors
  page.on('pageerror', err => {
    console.log(`PAGE ERROR: ${err.message}`);
  });
  
  console.log('Navigating to production site...');
  await page.goto('https://checkbox-grid-100x100.netlify.app');
  
  // Wait for page to load
  await page.waitForTimeout(3000);
  
  // Click the connect button to establish SpacetimeDB connection
  console.log('Clicking connect button...');
  await page.click('#connectBtn');
  
  // Wait for connection to establish (look for success message in console)
  console.log('Waiting for SpacetimeDB connection to establish...');
  await page.waitForTimeout(8000);
  
  // Try to click a checkbox to test interaction
  console.log('Attempting to click a checkbox after connection...');
  try {
    await page.click('canvas');
    console.log('Clicked canvas successfully');
  } catch (err) {
    console.log('Canvas click failed:', err.message);
  }
  
  // Wait a bit more to see if any async operations complete
  await page.waitForTimeout(3000);
  
  // Try clicking another spot to test persistence
  console.log('Attempting second click...');
  try {
    await page.click('canvas', { position: { x: 100, y: 100 } });
    console.log('Second click successful');
  } catch (err) {
    console.log('Second click failed:', err.message);
  }
  
  await page.waitForTimeout(3000);
  
  // Try the test button to trigger multiple checkbox changes
  console.log('Clicking test button to run pattern...');
  try {
    await page.click('#testBtn');
    console.log('Test button clicked');
    await page.waitForTimeout(5000); // Wait for test pattern to complete
  } catch (err) {
    console.log('Test button click failed:', err.message);
  }
  
  console.log('Test complete. Check console output above for SpacetimeDB functionality.');
  await browser.close();
})();
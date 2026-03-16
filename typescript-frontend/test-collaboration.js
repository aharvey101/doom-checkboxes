import { chromium } from 'playwright';

(async () => {
  console.log('🚀 Testing real-time collaboration between multiple browser tabs...');
  
  // Launch browser and create two tabs
  const browser = await chromium.launch({ headless: false, args: ['--disable-dev-shm-usage'] });
  const context1 = await browser.newContext();
  const context2 = await browser.newContext();
  const page1 = await context1.newPage();
  const page2 = await context2.newPage();
  
  // Set up console logging for both tabs
  page1.on('console', msg => console.log(`TAB1 ${msg.type()}: ${msg.text()}`));
  page2.on('console', msg => console.log(`TAB2 ${msg.type()}: ${msg.text()}`));
  
  // Navigate both tabs to the app
  console.log('Opening app in both tabs...');
  await Promise.all([
    page1.goto('https://checkbox-grid-100x100.netlify.app'),
    page2.goto('https://checkbox-grid-100x100.netlify.app')
  ]);
  
  // Wait for both pages to load
  await Promise.all([
    page1.waitForTimeout(3000),
    page2.waitForTimeout(3000)
  ]);
  
  // Connect both tabs to SpacetimeDB
  console.log('Connecting both tabs to SpacetimeDB...');
  await Promise.all([
    page1.click('#connectBtn'),
    page2.click('#connectBtn')
  ]);
  
  // Wait for connections to establish
  console.log('Waiting for connections...');
  await Promise.all([
    page1.waitForTimeout(8000),
    page2.waitForTimeout(8000)
  ]);
  
  // Test real-time collaboration
  console.log('Testing real-time sync: Tab 1 clicks checkbox...');
  await page1.click('canvas', { position: { x: 50, y: 50 } });
  
  console.log('Waiting for sync...');
  await page1.waitForTimeout(2000);
  
  console.log('Testing real-time sync: Tab 2 clicks checkbox...');
  await page2.click('canvas', { position: { x: 100, y: 100 } });
  
  console.log('Waiting for sync...');
  await page2.waitForTimeout(2000);
  
  console.log('Testing multiple rapid changes from Tab 1...');
  await page1.click('canvas', { position: { x: 150, y: 150 } });
  await page1.waitForTimeout(500);
  await page1.click('canvas', { position: { x: 200, y: 200 } });
  await page1.waitForTimeout(500);
  
  console.log('Final sync wait...');
  await page1.waitForTimeout(3000);
  
  console.log('✅ Collaboration test complete! Both tabs should show the same checkboxes.');
  console.log('📊 Check the console logs above to verify real-time synchronization worked.');
  
  await browser.close();
})();
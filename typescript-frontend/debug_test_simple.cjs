const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  
  // Capture console logs
  const logs = [];
  page.on('console', msg => {
    logs.push(`${msg.type()}: ${msg.text()}`);
  });
  
  // Capture WebSocket connections
  page.on('websocket', ws => {
    console.log('WebSocket opened:', ws.url());
    ws.on('close', () => console.log('WebSocket closed'));
  });
  
  await page.goto('http://localhost:8080');
  
  // Wait for page to load
  await page.waitForLoadState('networkidle');
  
  console.log('\n=== INITIAL CONSOLE LOGS ===');
  logs.slice(-10).forEach(log => console.log(log));
  
  // Click connect button
  const connectBtn = await page.locator('#connectBtn');
  if (await connectBtn.isVisible()) {
    console.log('\n=== CLICKING CONNECT BUTTON ===');
    await connectBtn.click();
    
    // Wait for connection attempt
    await page.waitForTimeout(3000);
    
    console.log('\n=== CONSOLE LOGS AFTER CONNECT ===');
    logs.slice(-15).forEach(log => console.log(log));
    
    // Check if test button is now enabled
    const testBtn = await page.locator('#testBtn');
    const isEnabled = await testBtn.isEnabled();
    
    console.log('\n=== TEST BUTTON STATUS ===');
    console.log('Test button enabled:', isEnabled);
    
    if (isEnabled) {
      console.log('\n=== CLICKING TEST BUTTON ===');
      await testBtn.click();
      await page.waitForTimeout(2000);
      
      console.log('\n=== CONSOLE LOGS AFTER TEST ===');
      logs.slice(-20).forEach(log => console.log(log));
    }
    
    // Try some checkbox operations
    console.log('\n=== TESTING CHECKBOX OPERATIONS ===');
    const canvas = await page.locator('#checkboxCanvas');
    if (await canvas.isVisible()) {
      await canvas.click();
      await page.keyboard.press('Space');
      await page.waitForTimeout(500);
      
      console.log('\n=== CONSOLE LOGS AFTER CHECKBOX TOGGLE ===');
      logs.slice(-10).forEach(log => console.log(log));
    }
  }
  
  await browser.close();
})();

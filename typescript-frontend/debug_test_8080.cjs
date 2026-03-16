const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  
  // Capture console logs
  const logs = [];
  page.on('console', msg => {
    logs.push(`${msg.type()}: ${msg.text()}`);
  });
  
  // Capture network requests
  const requests = [];
  page.on('request', req => {
    requests.push(`${req.method()} ${req.url()}`);
  });
  
  // Capture WebSocket connections
  const websockets = [];
  page.on('websocket', ws => {
    websockets.push(`WebSocket: ${ws.url()}`);
    ws.on('framereceived', frame => {
      console.log('WebSocket frame received:', frame.payload());
    });
    ws.on('framesent', frame => {
      console.log('WebSocket frame sent:', frame.payload());
    });
  });
  
  await page.goto('http://localhost:8080');
  
  // Wait for page to load
  await page.waitForLoadState('networkidle');
  
  console.log('\n=== INITIAL CONSOLE LOGS ===');
  logs.forEach(log => console.log(log));
  
  // Click connect button
  await page.click('#connectBtn');
  
  // Wait for connection
  await page.waitForTimeout(3000);
  
  console.log('\n=== CONSOLE LOGS AFTER CONNECT ===');
  logs.forEach(log => console.log(log));
  
  console.log('\n=== NETWORK REQUESTS ===');
  requests.forEach(req => console.log(req));
  
  console.log('\n=== WEBSOCKETS ===');
  websockets.forEach(ws => console.log(ws));
  
  // Try clicking test button if enabled
  const testBtn = await page.locator('#testBtn');
  const isEnabled = await testBtn.isEnabled();
  
  if (isEnabled) {
    console.log('\n=== CLICKING TEST BUTTON ===');
    await testBtn.click();
    await page.waitForTimeout(2000);
    
    console.log('\n=== CONSOLE LOGS AFTER TEST ===');
    logs.forEach(log => console.log(log));
  }
  
  await browser.close();
})();

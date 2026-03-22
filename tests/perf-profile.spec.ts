import { test, expect } from '@playwright/test';

test('profile cross-user sync performance', async ({ browser }) => {
  const player = await (await browser.newContext()).newPage();
  const spectator = await (await browser.newContext()).newPage();

  const perfLogs: string[] = [];

  // Capture PERF logs from spectator
  spectator.on('console', msg => {
    const text = msg.text();
    if (text.includes('[PERF')) {
      perfLogs.push(text);
      console.log('[SPECTATOR] ' + text);
    }
  });

  // Show player PERF and doom logs
  player.on('console', msg => {
    const text = msg.text();
    if (text.includes('[PERF')) {
      console.log('[PLAYER] ' + text);
    }
  });

  // Connect both
  await player.goto('http://127.0.0.1:8090');
  await player.waitForSelector('canvas', { timeout: 60000 });
  await player.waitForSelector('.status.connected', { timeout: 60000 });
  console.log('Player connected');

  await spectator.goto('http://127.0.0.1:8090');
  await spectator.waitForSelector('canvas', { timeout: 60000 });
  await spectator.waitForSelector('.status.connected', { timeout: 60000 });
  console.log('Spectator connected');

  // Navigate spectator to doom area
  await spectator.click('.doom-location-btn');
  console.log('Spectator at doom area');

  // Start Doom on player
  await player.click('.doom-btn');
  await player.waitForFunction(
    () => typeof (window as any).DoomMode !== 'undefined' && (window as any).DoomMode.isRunning(),
    { timeout: 20000 }
  );
  console.log('Doom running - watching perf logs for 15 seconds...\n');

  // Let it run and collect perf data
  await player.waitForTimeout(15000);

  console.log(`\n=== Collected ${perfLogs.length} PERF log entries from spectator ===`);
});

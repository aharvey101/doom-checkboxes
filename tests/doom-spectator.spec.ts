import { test, expect } from '@playwright/test';

test('spectator sees Doom frames', async ({ browser }) => {
  const player = await (await browser.newContext()).newPage();
  const spectator = await (await browser.newContext()).newPage();

  // Connect BOTH users first before starting Doom
  const url = process.env.BASE_URL || 'http://127.0.0.1:8080';
  await player.goto(url);
  await player.waitForSelector('canvas', { timeout: 15000 });
  await player.waitForSelector('.status.connected', { timeout: 30000 });
  console.log('STEP 1: Player connected');

  await spectator.goto(url);
  await spectator.waitForSelector('canvas', { timeout: 30000 });
  await spectator.waitForSelector('.status.connected', { timeout: 30000 });
  console.log('STEP 2: Spectator connected');

  // Navigate spectator to doom area first
  await spectator.click('.doom-location-btn');
  console.log('STEP 3: Spectator navigated to doom area');

  // Record spectator's doom data before Doom starts
  await spectator.waitForTimeout(2000);
  const beforeCount = await spectator.evaluate(() => {
    return (window as any).get_doom_chunk_nonzero_count?.() ?? -1;
  });
  console.log(`Spectator doom chunk BEFORE: ${beforeCount}`);

  // Now start Doom on the player
  await player.click('.doom-btn');
  await player.waitForFunction(
    () => typeof (window as any).DoomMode !== 'undefined' && (window as any).DoomMode.isRunning(),
    { timeout: 20000 }
  );
  console.log('STEP 4: Doom running on player');

  // Wait for initial frames to start flowing
  await player.waitForTimeout(5000);

  // Snapshot counters at start of measurement
  const startBatches = await spectator.evaluate(() => (window as any).get_delta_batch_count?.() ?? 0);
  const startUpdates = await spectator.evaluate(() => (window as any).get_delta_total_updates?.() ?? 0);
  const startTime = Date.now();

  // Let Doom run for 10 seconds
  await player.waitForTimeout(10000);

  // Snapshot counters at end
  const endBatches = await spectator.evaluate(() => (window as any).get_delta_batch_count?.() ?? 0);
  const endUpdates = await spectator.evaluate(() => (window as any).get_delta_total_updates?.() ?? 0);
  const elapsed = (Date.now() - startTime) / 1000;

  const batches = endBatches - startBatches;
  const pixelUpdates = endUpdates - startUpdates;
  const batchesPerSec = batches / elapsed;
  const updatesPerSec = pixelUpdates / elapsed;

  console.log(`\n=== SPECTATOR FRAMERATE BENCHMARK ===`);
  console.log(`Duration: ${elapsed.toFixed(1)}s`);
  console.log(`Delta batches received: ${batches} (${batchesPerSec.toFixed(1)}/sec)`);
  console.log(`Pixel updates applied: ${pixelUpdates} (${updatesPerSec.toFixed(0)}/sec)`);
  console.log(`Avg pixels per batch: ${batches > 0 ? (pixelUpdates / batches).toFixed(0) : 'N/A'}`);
  console.log(`=====================================\n`);

  // Spectator should receive at least some delta batches
  expect(batches).toBeGreaterThan(5);
});

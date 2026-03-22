import { test, expect } from '@playwright/test';

test('Doom e2e: WebSocket connects, Doom starts, renders frames', async ({ page }) => {
    await page.goto(process.env.BASE_URL || 'http://127.0.0.1:8080');
    await page.waitForSelector('canvas', { timeout: 15000 });

    // Assert 1: WebSocket connects
    console.log('\nWaiting for WebSocket connection...');
    await page.waitForSelector('.status.connected', { timeout: 15000 });
    console.log('✓ WebSocket connected');

    // Inject JS-side frame counter BEFORE starting doom so we intercept startCapture
    await page.evaluate(() => {
        (window as any)._doomFrameCount = 0;
        const orig = (window as any).DoomMode.startCapture;
        (window as any).DoomMode.startCapture = function(cb: any) {
            const wrapped = function(...args: any[]) {
                (window as any)._doomFrameCount++;
                return cb.apply(null, args);
            };
            return orig.call(this, wrapped);
        };
    });

    // Assert 2: Doom starts
    console.log('\nStarting Doom...');
    await page.click('.doom-btn');

    await page.waitForFunction(
        () => typeof (window as any).DoomMode !== 'undefined' && (window as any).DoomMode.isRunning(),
        { timeout: 20000 }
    );
    console.log('✓ Doom is running');

    // Assert 3: Doom frames are being processed through the rendering pipeline.
    // The doom WASM runs at 35 FPS internally. Frames flow through:
    //   doom.wasm → DoomJacobenget.onFrame → DoomMode.handleFrame → Rust callback
    //   → optimistic render to loaded_chunks → canvas repaint
    // We verify frames are flowing by counting callbacks over 5 seconds.
    console.log('\nMeasuring doom frame rate over 5 seconds...');
    const result = await page.evaluate(async () => {
        const start = (window as any)._doomFrameCount;
        await new Promise(r => setTimeout(r, 5000));
        const end = (window as any)._doomFrameCount;
        return { start, end, frames: end - start };
    });

    const fps = result.frames / 5;
    console.log(`Doom frames: ${result.frames} over 5s = ${fps.toFixed(1)} FPS`);

    // Doom targets 35 FPS. The clear_doom_chunks initial flush (256K updates)
    // and headless Chrome overhead reduce throughput. We assert > 5 FPS to
    // confirm the end-to-end pipeline is working.
    expect(fps).toBeGreaterThan(5);
    console.log(`✓ Doom rendering at ${fps.toFixed(1)} FPS`);
});

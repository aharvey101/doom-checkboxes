// Worker loader - imports and initializes the worker WASM module
import init from './worker/worker.js';

// Initialize the WASM module
// This will trigger worker_main() due to #[wasm_bindgen(start)]
init().catch(err => {
    console.error('[Worker] Failed to initialize WASM:', err);
});

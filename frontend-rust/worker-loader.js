// Worker loader - imports and initializes the worker WASM module
import init from './worker/worker.js';

// Buffer messages received before WASM is ready.
// worker_main() sets self.onmessage, so messages arriving before
// init() completes are lost. We intercept them here and replay after.
const pendingMessages = [];
self.onmessage = (event) => {
    pendingMessages.push(event.data);
};

// Initialize the WASM module
// This will trigger worker_main() due to #[wasm_bindgen(start)]
// worker_main() replaces self.onmessage with the real handler
init().then(() => {
    // Replay any messages that arrived during WASM init
    for (const data of pendingMessages) {
        self.onmessage(new MessageEvent('message', { data }));
    }
    pendingMessages.length = 0;
}).catch(err => {
    console.error('[Worker] Failed to initialize WASM:', err);
});

// DoomMode wrapper for DoomJacobenget
// Provides the interface expected by the Rust doom.rs module

window.DoomMode = (function() {
    let doomInstance = null;
    let currentCallback = null;
    let isCurrentlyRunning = false;
    let doomContainer = null;
    let previousFrame = null;

    // Doom rendering constants - matches doom.rs (original Doom resolution)
    const DOOM_WIDTH = 320;
    const DOOM_HEIGHT = 200;

    // Bayer 4x4 dither matrix for converting grayscale to binary
    const BAYER_MATRIX = [
        [0, 8, 2, 10],
        [12, 4, 14, 6],
        [3, 11, 1, 9],
        [15, 7, 13, 5]
    ];

    // Convert RGBA image data to dithered binary using Bayer 4x4
    function ditherToBinary(imageData) {
        const width = imageData.width;
        const height = imageData.height;
        const binary = new Uint8Array(width * height);

        for (let y = 0; y < height; y++) {
            for (let x = 0; x < width; x++) {
                const idx = y * width + x;
                const pixelIdx = idx * 4;

                // Convert to grayscale (using simple average)
                const r = imageData.data[pixelIdx];
                const g = imageData.data[pixelIdx + 1];
                const b = imageData.data[pixelIdx + 2];
                const gray = (r + g + b) / 3;

                // Apply Bayer dithering
                const threshold = (BAYER_MATRIX[y % 4][x % 4] / 16.0) * 255;
                binary[idx] = gray > threshold ? 1 : 0;
            }
        }

        return binary;
    }

    // Calculate delta between two binary frames
    function calculateDelta(newFrame, oldFrame) {
        const indices = [];
        const values = [];

        if (!oldFrame) {
            // First frame - return all pixels
            for (let i = 0; i < newFrame.length; i++) {
                indices.push(i);
                values.push(newFrame[i]);
            }
        } else {
            // Find changed pixels
            for (let i = 0; i < newFrame.length; i++) {
                if (newFrame[i] !== oldFrame[i]) {
                    indices.push(i);
                    values.push(newFrame[i]);
                }
            }
        }

        return { indices, values };
    }

    function handleFrame(imageData) {
        if (!currentCallback) return;

        // Convert to binary using dithering
        const binary = ditherToBinary(imageData);

        // Calculate delta
        const delta = calculateDelta(binary, previousFrame);

        // Store current frame for next delta
        previousFrame = binary;

        // Convert to JS arrays
        const indices = new Uint32Array(delta.indices);
        const values = new Uint8Array(delta.values);

        // Call the Rust callback with delta data
        // Parameters: indices, values, width, height, offset_x, offset_y
        const CHUNK_OFFSET_X = 5000;
        const CHUNK_OFFSET_Y = 5000;

        currentCallback(indices, values, DOOM_WIDTH, DOOM_HEIGHT, CHUNK_OFFSET_X, CHUNK_OFFSET_Y);
    }

    return {
        async init(containerId) {
            if (isCurrentlyRunning) {
                console.log("Doom already running");
                return Promise.resolve();
            }

            if (!window.DoomJacobenget) {
                throw new Error("DoomJacobenget not loaded");
            }

            // Get or create container
            doomContainer = document.getElementById(containerId);
            if (!doomContainer) {
                doomContainer = document.createElement('div');
                doomContainer.id = containerId;
                doomContainer.style.position = 'absolute';
                doomContainer.style.top = '0';
                doomContainer.style.left = '0';
                doomContainer.style.zIndex = '1000';
                doomContainer.style.display = 'none'; // Hidden by default
                document.body.appendChild(doomContainer);
            }

            console.log("Initializing Doom WASM...");

            try {
                // Create Doom instance
                doomInstance = new window.DoomJacobenget({
                    wasmURL: '/doom.wasm',
                    onFrame: handleFrame
                });

                // Load and initialize the game
                await doomInstance.loadGame();

                // Start the game loop
                doomInstance.startGameLoop();

                isCurrentlyRunning = true;
                console.log("Doom initialized successfully");

                // Set up keyboard event listeners
                document.addEventListener('keydown', (e) => {
                    if (isCurrentlyRunning && doomInstance) {
                        doomInstance.handleKeyDown(e);
                    }
                });

                document.addEventListener('keyup', (e) => {
                    if (isCurrentlyRunning && doomInstance) {
                        doomInstance.handleKeyUp(e);
                    }
                });

            } catch (error) {
                console.error("Failed to initialize Doom:", error);
                throw error;
            }
        },

        startCapture(callback) {
            console.log("Starting frame capture");
            currentCallback = callback;
            previousFrame = null; // Reset delta tracking
        },

        stopCapture() {
            console.log("Stopping frame capture");
            currentCallback = null;
            previousFrame = null;
        },

        stop() {
            console.log("Stopping Doom");
            if (doomInstance) {
                doomInstance.stopGameLoop();
                doomInstance = null;
            }
            currentCallback = null;
            previousFrame = null;
            isCurrentlyRunning = false;
        },

        isRunning() {
            return isCurrentlyRunning;
        },

        toggleControls() {
            // For now, controls are always enabled since we handle keyboard at document level
            return true;
        },

        enableControls() {
            // Controls are always enabled
        }
    };
})();

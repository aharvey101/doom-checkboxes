// DoomMode wrapper for DoomJacobenget
// Provides the interface expected by the Rust doom.rs module

window.DoomMode = (function() {
    let doomInstance = null;
    let currentCallback = null;
    let isCurrentlyRunning = false;
    let doomContainer = null;
    let previousFrame = null;
    let precompiledModule = null;

    // Precompile doom.wasm in the background on page load
    (async () => {
        try {
            const response = await fetch('/doom.wasm');
            const bytes = await response.arrayBuffer();
            precompiledModule = await WebAssembly.compile(bytes);
            console.log('Doom WASM precompiled and ready');
        } catch (e) {
            console.warn('Doom WASM precompile failed (will compile on demand):', e.message);
        }
    })();

    // Doom rendering constants - matches doom.rs and jacobenget/doom.wasm
    const DOOM_WIDTH = 640;
    const DOOM_HEIGHT = 400;

    // Color tolerance - only update if color difference exceeds this threshold
    // Higher = fewer updates but less accurate colors
    const COLOR_TOLERANCE = 80; // Sum of RGB differences (0-765 range)

    // Extract RGB colors from image data
    function extractColors(imageData) {
        const width = imageData.width;
        const height = imageData.height;
        const colors = new Uint8Array(width * height * 4); // r, g, b, brightness

        for (let i = 0; i < width * height; i++) {
            const pixelIdx = i * 4;
            const r = imageData.data[pixelIdx];
            const g = imageData.data[pixelIdx + 1];
            const b = imageData.data[pixelIdx + 2];

            // Calculate brightness for checked state
            const brightness = (r + g + b) / 3;

            colors[pixelIdx] = r;
            colors[pixelIdx + 1] = g;
            colors[pixelIdx + 2] = b;
            colors[pixelIdx + 3] = brightness > 64 ? 1 : 0; // Use brightness threshold for checked
        }

        return colors;
    }

    // Calculate delta between two color frames
    function calculateDelta(newFrame, oldFrame) {
        const indices = [];
        const colors = []; // Will contain [r, g, b, checked] for each changed pixel

        if (!oldFrame) {
            // First frame - return all pixels
            for (let i = 0; i < newFrame.length / 4; i++) {
                indices.push(i);
                colors.push([
                    newFrame[i * 4],     // r
                    newFrame[i * 4 + 1], // g
                    newFrame[i * 4 + 2], // b
                    newFrame[i * 4 + 3]  // checked
                ]);
            }
        } else {
            // Find changed pixels with color tolerance
            for (let i = 0; i < newFrame.length / 4; i++) {
                const idx = i * 4;

                // Calculate color difference
                const rDiff = Math.abs(newFrame[idx] - oldFrame[idx]);
                const gDiff = Math.abs(newFrame[idx + 1] - oldFrame[idx + 1]);
                const bDiff = Math.abs(newFrame[idx + 2] - oldFrame[idx + 2]);
                const colorDiff = rDiff + gDiff + bDiff;

                // Check if checked state changed
                const checkedChanged = newFrame[idx + 3] !== oldFrame[idx + 3];

                // Only update if color difference exceeds tolerance OR checked state changed
                if (colorDiff > COLOR_TOLERANCE || checkedChanged) {
                    indices.push(i);
                    colors.push([
                        newFrame[idx],     // r
                        newFrame[idx + 1], // g
                        newFrame[idx + 2], // b
                        newFrame[idx + 3]  // checked
                    ]);
                }
            }
        }

        return { indices, colors };
    }

    function handleFrame(imageData) {
        if (!currentCallback) return;

        // Extract RGB colors from the frame
        const colorData = extractColors(imageData);

        // Calculate delta
        const delta = calculateDelta(colorData, previousFrame);

        // Store current frame for next delta
        previousFrame = colorData;

        // Convert to JS arrays
        const indices = new Uint32Array(delta.indices);

        // Pack colors into a single array: [r, g, b, checked, r, g, b, checked, ...]
        const packedColors = new Uint8Array(delta.colors.length * 4);
        for (let i = 0; i < delta.colors.length; i++) {
            packedColors[i * 4] = delta.colors[i][0];     // r
            packedColors[i * 4 + 1] = delta.colors[i][1]; // g
            packedColors[i * 4 + 2] = delta.colors[i][2]; // b
            packedColors[i * 4 + 3] = delta.colors[i][3]; // checked
        }

        // Call the Rust callback with delta data including colors
        // Parameters: indices, packedColors, width, height, offset_x, offset_y
        const CHUNK_OFFSET_X = 5000;
        const CHUNK_OFFSET_Y = 5000;

        currentCallback(indices, packedColors, DOOM_WIDTH, DOOM_HEIGHT, CHUNK_OFFSET_X, CHUNK_OFFSET_Y);
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
                // Create Doom instance (use precompiled module if available)
                doomInstance = new window.DoomJacobenget({
                    wasmURL: '/doom.wasm',
                    precompiledModule: precompiledModule,
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

console.log('✓ DoomMode loaded successfully');

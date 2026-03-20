// Doom WASM integration using jacobenget/doom.wasm
// This provides a clean, minimal interface for running Doom

class DoomJacobenget {
    constructor(options) {
        this.onFrame = options.onFrame || (() => {});
        this.moduleInstanceMemory = null;
        this.exports = null;
        this.wasmURL = options.wasmURL || '/doom.wasm';
        this.doomWidth = 0;
        this.doomHeight = 0;
        this.keyMap = new Map();
    }

    async loadGame() {
        // Define all the imports that doom.wasm needs
        const imports = {
            "loading": {
                "onGameInit": (width, height) => {
                    this.doomWidth = width;
                    this.doomHeight = height;
                    console.log(`Doom initialized: ${width}x${height}`);
                },
                // Provide no WAD data, so the module defaults to using the Doom Shareware WAD
                "wadSizes": () => {},
                "readWads": () => {},
            },
            "ui": {
                "drawFrame": (indexOfFrameBuffer) => {
                    // Get the frame buffer from WASM memory
                    const doomFrameBuffer = new Uint8Array(
                        this.moduleInstanceMemory.buffer,
                        indexOfFrameBuffer,
                        this.doomWidth * this.doomHeight * 4
                    );

                    // Convert BGRA (little-endian ARGB) to RGBA
                    const rgba = new Uint8ClampedArray(this.doomWidth * this.doomHeight * 4);
                    for (let i = 0; i < (this.doomWidth * this.doomHeight); i++) {
                        rgba[4*i+0] = doomFrameBuffer[4*i+2];  // Red
                        rgba[4*i+1] = doomFrameBuffer[4*i+1];  // Green
                        rgba[4*i+2] = doomFrameBuffer[4*i+0];  // Blue
                        rgba[4*i+3] = 255;  // Alpha (fully opaque)
                    }

                    // WASM outputs 640x400, but we want 320x200 for checkboxes
                    // Scale down by 2x using canvas
                    const fullImageData = new ImageData(rgba, this.doomWidth, this.doomHeight);

                    if (!this.scalingCanvas) {
                        this.scalingCanvas = document.createElement('canvas');
                        this.scalingCanvas.width = this.doomWidth / 2;
                        this.scalingCanvas.height = this.doomHeight / 2;
                        this.scalingCtx = this.scalingCanvas.getContext('2d');
                    }

                    // Create temp canvas with full resolution
                    if (!this.tempCanvas) {
                        this.tempCanvas = document.createElement('canvas');
                        this.tempCanvas.width = this.doomWidth;
                        this.tempCanvas.height = this.doomHeight;
                        this.tempCtx = this.tempCanvas.getContext('2d');
                    }

                    // Draw full resolution to temp canvas
                    this.tempCtx.putImageData(fullImageData, 0, 0);

                    // Scale down to target resolution
                    this.scalingCtx.drawImage(
                        this.tempCanvas,
                        0, 0, this.doomWidth, this.doomHeight,
                        0, 0, this.doomWidth / 2, this.doomHeight / 2
                    );

                    // Get the scaled down image
                    const scaledImageData = this.scalingCtx.getImageData(
                        0, 0, this.doomWidth / 2, this.doomHeight / 2
                    );

                    // Call the frame callback with scaled ImageData
                    this.onFrame(scaledImageData);
                },
            },
            "runtimeControl": {
                "timeInMilliseconds": () => BigInt(Math.trunc(performance.now()))
            },
            "console": {
                "onInfoMessage": (messagePtr, length) => {
                    const message = this.readMemoryAsString(messagePtr, length);
                    console.log(`[Doom] ${message}`);
                },
                "onErrorMessage": (messagePtr, length) => {
                    const message = this.readMemoryAsString(messagePtr, length);
                    console.error(`[Doom Error] ${message}`);
                },
            },
            "gameSaving": {
                // No support for saving games
                "sizeOfSaveGame": () => 0,
                "readSaveGame": () => 0,
                "writeSaveGame": () => 0,
            },
        };

        // Load and instantiate the WASM module
        const { instance } = await WebAssembly.instantiateStreaming(fetch(this.wasmURL), imports);

        this.exports = instance.exports;
        this.moduleInstanceMemory = this.exports.memory;

        // Set up key mappings
        this.keyMap = new Map([
            ["ArrowLeft", this.exports.KEY_LEFTARROW],
            ["ArrowRight", this.exports.KEY_RIGHTARROW],
            ["ArrowUp", this.exports.KEY_UPARROW],
            ["ArrowDown", this.exports.KEY_DOWNARROW],
            [",", this.exports.KEY_STRAFE_L],
            [".", this.exports.KEY_STRAFE_R],
            ["Control", this.exports.KEY_FIRE],
            [" ", this.exports.KEY_USE],
            ["Shift", this.exports.KEY_SHIFT],
            ["Tab", this.exports.KEY_TAB],
            ["Escape", this.exports.KEY_ESCAPE],
            ["Enter", this.exports.KEY_ENTER],
            ["Backspace", this.exports.KEY_BACKSPACE],
            ["Alt", this.exports.KEY_ALT],
        ]);

        // Initialize Doom
        this.exports.initGame();

        console.log("Doom WASM module loaded and initialized");
    }

    readMemoryAsString(offset, length) {
        const buffer8 = new Uint8Array(this.moduleInstanceMemory.buffer);
        const decoder = new TextDecoder("utf-8", { fatal: false });
        const data = buffer8.slice(offset, offset + length);
        return decoder.decode(data);
    }

    startGameLoop() {
        // Doom likes to run at 35 FPS
        this.gameLoopInterval = setInterval(() => {
            if (this.exports) {
                this.exports.tickGame();
            }
        }, 1000 / 35);
        console.log("Doom game loop started (35 FPS)");
    }

    stopGameLoop() {
        if (this.gameLoopInterval) {
            clearInterval(this.gameLoopInterval);
            this.gameLoopInterval = null;
            console.log("Doom game loop stopped");
        }
    }

    handleKeyDown(event) {
        const doomKey = this.convertKeyEventToDoomKey(event);
        if (doomKey !== null && this.exports) {
            this.exports.reportKeyDown(doomKey);
            return true; // Key was handled
        }
        return false;
    }

    handleKeyUp(event) {
        const doomKey = this.convertKeyEventToDoomKey(event);
        if (doomKey !== null && this.exports) {
            this.exports.reportKeyUp(doomKey);
            return true; // Key was handled
        }
        return false;
    }

    convertKeyEventToDoomKey(event) {
        if (this.keyMap.has(event.key)) {
            return this.keyMap.get(event.key);
        } else if (event.key.length === 1) {
            // Single character keys use their ASCII/Unicode value
            return event.key.charCodeAt(0);
        }
        return null;
    }

    getDimensions() {
        return { width: this.doomWidth, height: this.doomHeight };
    }
}

// Export for use in other modules
window.DoomJacobenget = DoomJacobenget;

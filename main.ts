import { DbConnection } from "./generated";

// Configuration
const isLocal = window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1";
const SPACETIMEDB_URI = isLocal ? "ws://127.0.0.1:3000" : "wss://maincloud.spacetimedb.com";
const DATABASE_NAME = "checkboxes";

// Grid configuration: 1000x1000 = 1 million checkboxes
const GRID_WIDTH = 1000;
const GRID_HEIGHT = 1000;
const TOTAL_CHECKBOXES = GRID_WIDTH * GRID_HEIGHT;
const CELL_SIZE = 4; // pixels per checkbox

// Colors
const COLOR_CHECKED = "#2ecc71";
const COLOR_UNCHECKED = "#2c3e50";
const COLOR_GRID = "#1a1a2e";

// DOM elements
const statusEl = document.getElementById("status")!;
const canvasEl = document.getElementById("checkbox-canvas") as HTMLCanvasElement;
const statsEl = document.getElementById("stats")!;
const ctx = canvasEl.getContext("2d")!;

// State
let conn: DbConnection | null = null;
let chunkData: Uint8Array = new Uint8Array(125000); // 1M bits
let checkedCount = 0;

// Viewport for pan/zoom
let offsetX = 0;
let offsetY = 0;
let scale = 1;
let isDragging = false;
let lastMouseX = 0;
let lastMouseY = 0;

// Bit manipulation
function getBit(data: Uint8Array, bitIndex: number): boolean {
  const byteIdx = Math.floor(bitIndex / 8);
  const bitIdx = bitIndex % 8;
  return byteIdx < data.length ? ((data[byteIdx] >> bitIdx) & 1) === 1 : false;
}

function countChecked(data: Uint8Array): number {
  let count = 0;
  for (let i = 0; i < TOTAL_CHECKBOXES; i++) {
    if (getBit(data, i)) count++;
  }
  return count;
}

// Rendering
function render() {
  const width = canvasEl.width;
  const height = canvasEl.height;
  
  // Clear canvas
  ctx.fillStyle = COLOR_GRID;
  ctx.fillRect(0, 0, width, height);
  
  // Calculate visible range
  const cellSize = CELL_SIZE * scale;
  const startCol = Math.max(0, Math.floor(-offsetX / cellSize));
  const startRow = Math.max(0, Math.floor(-offsetY / cellSize));
  const endCol = Math.min(GRID_WIDTH, Math.ceil((width - offsetX) / cellSize));
  const endRow = Math.min(GRID_HEIGHT, Math.ceil((height - offsetY) / cellSize));
  
  // Draw visible checkboxes
  for (let row = startRow; row < endRow; row++) {
    for (let col = startCol; col < endCol; col++) {
      const bitIndex = row * GRID_WIDTH + col;
      const isChecked = getBit(chunkData, bitIndex);
      
      const x = offsetX + col * cellSize;
      const y = offsetY + row * cellSize;
      
      ctx.fillStyle = isChecked ? COLOR_CHECKED : COLOR_UNCHECKED;
      ctx.fillRect(x + 0.5, y + 0.5, cellSize - 1, cellSize - 1);
    }
  }
}

// Convert canvas coordinates to grid coordinates
function canvasToGrid(canvasX: number, canvasY: number): { col: number; row: number } | null {
  const cellSize = CELL_SIZE * scale;
  const col = Math.floor((canvasX - offsetX) / cellSize);
  const row = Math.floor((canvasY - offsetY) / cellSize);
  
  if (col >= 0 && col < GRID_WIDTH && row >= 0 && row < GRID_HEIGHT) {
    return { col, row };
  }
  return null;
}

// Handle canvas click
function handleClick(e: MouseEvent) {
  if (!conn) return;
  
  const rect = canvasEl.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const y = e.clientY - rect.top;
  
  const grid = canvasToGrid(x, y);
  if (grid) {
    const bitIndex = grid.row * GRID_WIDTH + grid.col;
    const currentState = getBit(chunkData, bitIndex);
    
    conn.reducers.updateCheckbox({
      chunkId: 0,
      bitOffset: bitIndex,
      checked: !currentState,
    });
  }
}

// Pan handlers
function handleMouseDown(e: MouseEvent) {
  if (e.button === 0 && e.shiftKey) {
    isDragging = true;
    lastMouseX = e.clientX;
    lastMouseY = e.clientY;
    canvasEl.style.cursor = "grabbing";
  }
}

function handleMouseMove(e: MouseEvent) {
  if (isDragging) {
    const dx = e.clientX - lastMouseX;
    const dy = e.clientY - lastMouseY;
    offsetX += dx;
    offsetY += dy;
    lastMouseX = e.clientX;
    lastMouseY = e.clientY;
    render();
  }
}

function handleMouseUp() {
  isDragging = false;
  canvasEl.style.cursor = "crosshair";
}

// Zoom handler
function handleWheel(e: WheelEvent) {
  e.preventDefault();
  
  const rect = canvasEl.getBoundingClientRect();
  const mouseX = e.clientX - rect.left;
  const mouseY = e.clientY - rect.top;
  
  const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
  const newScale = Math.max(0.5, Math.min(10, scale * zoomFactor));
  
  // Zoom toward mouse position
  const scaleChange = newScale / scale;
  offsetX = mouseX - (mouseX - offsetX) * scaleChange;
  offsetY = mouseY - (mouseY - offsetY) * scaleChange;
  scale = newScale;
  
  render();
}

// Resize handler
function handleResize() {
  canvasEl.width = window.innerWidth - 40;
  canvasEl.height = window.innerHeight - 120;
  render();
}

// Update stats display
function updateStats() {
  statsEl.textContent = `${checkedCount.toLocaleString()} / ${TOTAL_CHECKBOXES.toLocaleString()} checked | Zoom: ${scale.toFixed(1)}x | Shift+drag to pan, scroll to zoom`;
}

// Update from chunk data
function updateFromChunk(state: Uint8Array) {
  chunkData = new Uint8Array(state);
  checkedCount = countChecked(chunkData);
  updateStats();
  render();
}

// Set status
function setStatus(text: string, type: "connecting" | "connected" | "error") {
  statusEl.textContent = text;
  statusEl.className = `status ${type}`;
}

// Connect to SpacetimeDB
async function connect() {
  setStatus(`Connecting to ${isLocal ? "local" : "production"}...`, "connecting");

  try {
    conn = await DbConnection.builder()
      .withUri(SPACETIMEDB_URI)
      .withDatabaseName(DATABASE_NAME)
      .onConnect((connection, identity) => {
        console.log("Connected with identity:", identity.toHexString());
        setStatus("Connected - subscribing...", "connecting");

        // Register table listeners BEFORE subscribing
        connection.db.checkbox_chunk.onInsert((_ctx, row) => {
          console.log("Chunk inserted:", row.chunkId);
          if (row.chunkId === 0) {
            updateFromChunk(row.state);
          }
        });

        connection.db.checkbox_chunk.onUpdate((_ctx, _oldRow, newRow) => {
          console.log("Chunk updated:", newRow.chunkId, "version:", newRow.version);
          if (newRow.chunkId === 0) {
            updateFromChunk(newRow.state);
          }
        });

        // Now subscribe
        connection
          .subscriptionBuilder()
          .onApplied(() => {
            console.log("Subscription applied");
            setStatus("Connected", "connected");

            for (const chunk of connection.db.checkbox_chunk.iter()) {
              if (chunk.chunkId === 0) {
                updateFromChunk(chunk.state);
              }
            }
          })
          .onError((_ctx, error) => {
            console.error("Subscription error:", error);
            setStatus("Subscription error", "error");
          })
          .subscribe("SELECT * FROM checkbox_chunk");
      })
      .onDisconnect(() => {
        console.log("Disconnected");
        setStatus("Disconnected", "error");
      })
      .onConnectError((error) => {
        console.error("Connection error:", error);
        setStatus("Connection failed", "error");
      })
      .build();
  } catch (error) {
    console.error("Failed to connect:", error);
    setStatus("Connection failed: " + String(error), "error");
  }
}

// Initialize
function init() {
  // Set up canvas
  handleResize();
  window.addEventListener("resize", handleResize);
  
  // Set up interactions
  canvasEl.addEventListener("click", handleClick);
  canvasEl.addEventListener("mousedown", handleMouseDown);
  canvasEl.addEventListener("mousemove", handleMouseMove);
  canvasEl.addEventListener("mouseup", handleMouseUp);
  canvasEl.addEventListener("mouseleave", handleMouseUp);
  canvasEl.addEventListener("wheel", handleWheel, { passive: false });
  canvasEl.style.cursor = "crosshair";
  
  // Initial render
  render();
  updateStats();
  
  // Connect
  connect();
}

init();

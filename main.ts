import { DbConnection, SubscriptionBuilder } from "./generated";

// Configuration
const SPACETIMEDB_URI = "ws://127.0.0.1:3000";
const DATABASE_NAME = "checkboxes";
const CHECKBOX_COUNT = 500; // 50x10 grid for demo

// DOM elements
const statusEl = document.getElementById("status")!;
const gridEl = document.getElementById("checkbox-grid")!;
const statsEl = document.getElementById("stats")!;

// Connection state
let conn: DbConnection | null = null;
let checkboxElements: HTMLInputElement[] = [];

// Bit manipulation helpers
function getBit(data: Uint8Array, bitIndex: number): boolean {
  const byteIdx = Math.floor(bitIndex / 8);
  const bitIdx = bitIndex % 8;
  return byteIdx < data.length ? ((data[byteIdx] >> bitIdx) & 1) === 1 : false;
}

// Create checkbox elements
function createCheckboxGrid() {
  gridEl.innerHTML = "";
  checkboxElements = [];

  for (let i = 0; i < CHECKBOX_COUNT; i++) {
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.dataset.index = String(i);

    checkbox.addEventListener("change", () => {
      if (!conn) return;

      const bitOffset = i;
      const chunkId = 0; // All in chunk 0 for now

      // Call reducer with object-style args (SpacetimeDB v2.0)
      conn.reducers.updateCheckbox({
        chunkId,
        bitOffset,
        checked: checkbox.checked,
      });
    });

    gridEl.appendChild(checkbox);
    checkboxElements.push(checkbox);
  }
}

// Update checkboxes from chunk data
function updateCheckboxesFromChunk(chunkId: number, state: Uint8Array) {
  if (chunkId !== 0) return; // Only handling chunk 0 for now

  let checkedCount = 0;
  for (let i = 0; i < CHECKBOX_COUNT; i++) {
    const isChecked = getBit(state, i);
    if (checkboxElements[i]) {
      checkboxElements[i].checked = isChecked;
      if (isChecked) checkedCount++;
    }
  }

  statsEl.textContent = `${checkedCount} / ${CHECKBOX_COUNT} checked`;
}

// Set status display
function setStatus(text: string, type: "connecting" | "connected" | "error") {
  statusEl.textContent = text;
  statusEl.className = `status ${type}`;
}

// Connect to SpacetimeDB
async function connect() {
  setStatus("Connecting...", "connecting");
  createCheckboxGrid();

  try {
    conn = await DbConnection.builder()
      .withUri(SPACETIMEDB_URI)
      .withDatabaseName(DATABASE_NAME)
      .onConnect((connection, identity) => {
        console.log("Connected with identity:", identity.toHexString());
        setStatus("Connected - subscribing...", "connecting");

        // Subscribe to checkbox_chunk table
        connection
          .subscriptionBuilder()
          .onApplied(() => {
            console.log("Subscription applied");
            setStatus("Connected", "connected");

            // Load initial state from any existing chunks
            for (const chunk of connection.db.checkbox_chunk.iter()) {
              updateCheckboxesFromChunk(chunk.chunkId, chunk.state);
            }
          })
          .onError((ctx, error) => {
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

    // Listen for table updates
    conn.db.checkbox_chunk.onInsert((ctx, row) => {
      console.log("Chunk inserted:", row.chunkId);
      updateCheckboxesFromChunk(row.chunkId, row.state);
    });

    conn.db.checkbox_chunk.onUpdate((ctx, oldRow, newRow) => {
      updateCheckboxesFromChunk(newRow.chunkId, newRow.state);
    });
  } catch (error) {
    console.error("Failed to connect:", error);
    setStatus("Connection failed: " + String(error), "error");
  }
}

// Start
connect();

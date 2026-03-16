import { describe, test, beforeEach, afterEach, expect, vi, type Mock } from 'vitest';
import { SpacetimeDBCheckboxApp } from '../src/SpacetimeDBCheckboxApp.js';

describe('Checkbox WebSocket Integration', () => {
  let app: SpacetimeDBCheckboxApp;
  let canvas: HTMLCanvasElement;
  let mockUpdateCheckbox: Mock;
  let mockAddChunk: Mock;

  beforeEach(async () => {
    // Create DOM elements
    canvas = document.createElement('canvas');
    canvas.width = 800;
    canvas.height = 600;
    document.body.appendChild(canvas);

    // Create app instance
    app = new SpacetimeDBCheckboxApp();

    // Initialize the app to set up the database connection
    app.initializeCanvas(canvas);
    
    // Wait for initialization and connection
    await new Promise(resolve => setTimeout(resolve, 100));

    // Spy on the database methods after the app is initialized
    mockUpdateCheckbox = vi.spyOn(app['checkboxDatabase'], 'updateCheckbox').mockResolvedValue(undefined);
    mockAddChunk = vi.spyOn(app['checkboxDatabase'], 'addChunk').mockResolvedValue(undefined);
    
    // Mock isConnected to return true
    vi.spyOn(app['checkboxDatabase'], 'isConnected').mockReturnValue(true);
  });

  afterEach(() => {
    document.body.removeChild(canvas);
    vi.clearAllMocks();
  });

  test('should send WebSocket message to SpacetimeDB when toggling checkbox', async () => {
    // Toggle checkbox at position (0, 0)
    await app.toggleCheckbox(0, 0);

    // Verify that updateCheckbox was called with correct parameters
    expect(mockUpdateCheckbox).toHaveBeenCalledTimes(1);
    expect(mockUpdateCheckbox).toHaveBeenCalledWith(
      0,        // chunkId: row * gridSize + col = 0 * 100 + 0 = 0, divided by 1000000 = 0
      0,        // bitOffset: globalIndex % 1000000 = 0
      true      // checked: should be true for first toggle
    );
  });

  test('should send WebSocket message for checkbox in different grid positions', async () => {
    // Toggle checkbox at position (5, 10) - this should calculate different globalIndex
    await app.toggleCheckbox(10, 5); // Note: method expects (x, y) but internally uses (row, col)

    // Calculate expected values:
    // row = 5, col = 10
    // globalIndex = 5 * 100 + 10 = 510
    // chunkId = Math.floor(510 / 1000000) = 0
    // bitOffset = 510 % 1000000 = 510
    
    expect(mockUpdateCheckbox).toHaveBeenCalledWith(0, 510, true);
  });

  test('should toggle checkbox state from false to true to false', async () => {
    // First toggle: false -> true
    await app.toggleCheckbox(0, 0);
    expect(mockUpdateCheckbox).toHaveBeenNthCalledWith(1, 0, 0, true);

    // Second toggle: true -> false 
    await app.toggleCheckbox(0, 0);
    expect(mockUpdateCheckbox).toHaveBeenNthCalledWith(2, 0, 0, false);

    // Third toggle: false -> true
    await app.toggleCheckbox(0, 0);
    expect(mockUpdateCheckbox).toHaveBeenNthCalledWith(3, 0, 0, true);

    expect(mockUpdateCheckbox).toHaveBeenCalledTimes(3);
  });

  test('should call addChunk when creating new chunk for first time', async () => {
    // Toggle checkbox - this should trigger chunk creation since it's empty
    await app.toggleCheckbox(0, 0);

    // Should call addChunk for chunkId 0 since it's a new chunk
    expect(mockAddChunk).toHaveBeenCalledTimes(1);
    expect(mockAddChunk).toHaveBeenCalledWith(0);
    
    // Should also call updateCheckbox
    expect(mockUpdateCheckbox).toHaveBeenCalledTimes(1);
  });
});
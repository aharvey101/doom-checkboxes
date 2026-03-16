import { describe, test, beforeEach, afterEach, expect, vi, type Mock } from 'vitest';
import { SpacetimeDBCheckboxApp } from '../src/SpacetimeDBCheckboxApp.js';

describe('Checkbox Persistence & Synchronization', () => {
  let app: SpacetimeDBCheckboxApp;
  let canvas: HTMLCanvasElement;
  let mockGetAllChunks: Mock;
  let mockGetChunkById: Mock;

  beforeEach(async () => {
    // Create DOM elements
    canvas = document.createElement('canvas');
    canvas.width = 800;
    canvas.height = 600;
    document.body.appendChild(canvas);

    // Create app instance
    app = new SpacetimeDBCheckboxApp();

    // Initialize the app
    app.initializeCanvas(canvas);
    await new Promise(resolve => setTimeout(resolve, 100));

    // Mock database methods for data loading
    mockGetAllChunks = vi.spyOn(app['checkboxDatabase'], 'getAllChunks');
    mockGetChunkById = vi.spyOn(app['checkboxDatabase'], 'getChunkById');
    
    // Mock connection status
    vi.spyOn(app['checkboxDatabase'], 'isConnected').mockReturnValue(true);
  });

  afterEach(() => {
    document.body.removeChild(canvas);
    vi.clearAllMocks();
  });

  test('should load existing checkbox data from database on initialization', async () => {
    // Mock database returning existing chunk data
    const mockChunkData = [{
      chunkId: 0,
      state: new Uint8Array([0b00000001, 0, 0, 0]), // First checkbox checked
      version: 1
    }];
    mockGetAllChunks.mockResolvedValue(mockChunkData);

    // Create a new app instance to test initialization
    const newApp = new SpacetimeDBCheckboxApp();
    newApp.initializeCanvas(canvas);
    
    // Connect to database (this should trigger loadInitialState)
    vi.spyOn(newApp['checkboxDatabase'], 'connect').mockResolvedValue(true);
    vi.spyOn(newApp['checkboxDatabase'], 'getAllChunks').mockResolvedValue(mockChunkData);
    
    await newApp.connect();
    
    // Wait for initialization to complete
    await new Promise(resolve => setTimeout(resolve, 100));

    // Verify getAllChunks was called during initialization
    expect(newApp['checkboxDatabase'].getAllChunks).toHaveBeenCalled();

    // Verify the checkbox state was loaded from the database
    const checkboxState = newApp.getCheckboxState(0, 0);
    expect(checkboxState).toBe(true); // Should be true from mocked data
  });

  test('should load checkbox state correctly after page refresh simulation', async () => {
    // First, connect and set up a checkbox
    vi.spyOn(app['checkboxDatabase'], 'connect').mockResolvedValue(true);
    await app.connect();
    await app.toggleCheckbox(0, 0);
    
    // Simulate page refresh by creating new app instance
    const newApp = new SpacetimeDBCheckboxApp();
    
    // Mock the database to return the state from the previous session
    const mockChunkData = [{
      chunkId: 0,
      state: new Uint8Array([0b00000001]), // First checkbox checked
      version: 1
    }];
    vi.spyOn(newApp['checkboxDatabase'], 'connect').mockResolvedValue(true);
    vi.spyOn(newApp['checkboxDatabase'], 'getAllChunks').mockResolvedValue(mockChunkData);
    vi.spyOn(newApp['checkboxDatabase'], 'isConnected').mockReturnValue(true);

    // Initialize the new app (simulating page reload)
    newApp.initializeCanvas(canvas);
    await newApp.connect(); // This should load the data
    await new Promise(resolve => setTimeout(resolve, 100));

    // The state should persist across "page reloads"
    const persistedState = newApp.getCheckboxState(0, 0);
    expect(persistedState).toBe(true);
  });

  test('should handle real-time updates from other users', async () => {
    // First, connect the app to set up subscriptions
    vi.spyOn(app['checkboxDatabase'], 'connect').mockResolvedValue(true);
    await app.connect();

    // Simulate receiving an update from another user via subscription
    const mockChunkUpdate = {
      chunkId: 0,
      state: new Uint8Array([0b00000010]), // Second checkbox checked
      version: 2
    };

    // Get the update callback that was registered during initialization
    const updateCallbacks = app['checkboxDatabase']['updateCallbacks'];
    expect(updateCallbacks).toBeDefined();
    expect(updateCallbacks.length).toBeGreaterThan(0);

    // Simulate receiving an update from SpacetimeDB
    updateCallbacks.forEach(callback => callback(mockChunkUpdate));

    // Wait for the update to be processed
    await new Promise(resolve => setTimeout(resolve, 100));

    // The checkbox state should reflect the update from another user
    const updatedState = app.getCheckboxState(0, 1);
    expect(updatedState).toBe(true);
  });

  test('should identify missing data loading if getAllChunks is not called', async () => {
    // This test will identify if the app fails to load data on startup
    
    // Create a fresh app instance
    const freshApp = new SpacetimeDBCheckboxApp();
    
    // Mock connection methods
    const mockConnect = vi.spyOn(freshApp['checkboxDatabase'], 'connect').mockResolvedValue(true);
    const mockGetAllChunks = vi.spyOn(freshApp['checkboxDatabase'], 'getAllChunks').mockResolvedValue([]);
    vi.spyOn(freshApp['checkboxDatabase'], 'isConnected').mockReturnValue(true);
    
    // Initialize the app
    freshApp.initializeCanvas(canvas);
    await freshApp.connect(); // Connect explicitly
    await new Promise(resolve => setTimeout(resolve, 100));

    // If this fails, it means the app is not loading data on connection
    expect(mockGetAllChunks).toHaveBeenCalled();
  });
});
import { describe, test, beforeEach, afterEach, expect, vi, type Mock } from 'vitest';
import { SpacetimeDBCheckboxApp } from '../src/SpacetimeDBCheckboxApp.js';

describe('Checkbox Auto-Connection', () => {
  let canvas: HTMLCanvasElement;

  beforeEach(() => {
    // Create DOM elements
    canvas = document.createElement('canvas');
    canvas.width = 800;
    canvas.height = 600;
    document.body.appendChild(canvas);
  });

  afterEach(() => {
    document.body.removeChild(canvas);
    vi.clearAllMocks();
  });

  test('should automatically connect to SpacetimeDB when canvas is initialized', async () => {
    const app = new SpacetimeDBCheckboxApp();
    
    // Mock database methods
    const mockConnect = vi.spyOn(app, 'connect').mockResolvedValue(true);
    vi.spyOn(app['checkboxDatabase'], 'connect').mockResolvedValue(true);
    vi.spyOn(app['checkboxDatabase'], 'getAllChunks').mockResolvedValue([]);
    vi.spyOn(app['checkboxDatabase'], 'isConnected').mockReturnValue(true);
    
    // Initialize canvas - this should trigger auto-connect
    app.initializeCanvas(canvas);
    
    // Wait for auto-connect to complete
    await new Promise(resolve => setTimeout(resolve, 100));
    
    // Verify connect was called automatically
    expect(mockConnect).toHaveBeenCalled();
  });

  test('should load existing data immediately after auto-connection', async () => {
    const app = new SpacetimeDBCheckboxApp();
    
    // Mock database returning existing data
    const mockChunkData = [{
      chunkId: 0,
      state: new Uint8Array([0b00000001]), // First checkbox checked
      version: 1
    }];
    
    vi.spyOn(app['checkboxDatabase'], 'connect').mockResolvedValue(true);
    vi.spyOn(app['checkboxDatabase'], 'getAllChunks').mockResolvedValue(mockChunkData);
    vi.spyOn(app['checkboxDatabase'], 'isConnected').mockReturnValue(true);
    
    // Initialize canvas
    app.initializeCanvas(canvas);
    
    // Wait for auto-connect and data loading
    await new Promise(resolve => setTimeout(resolve, 200));
    
    // Data should be available immediately without manual connection
    const checkboxState = app.getCheckboxState(0, 0);
    expect(checkboxState).toBe(true);
  });

  test('should handle auto-connection failures gracefully', async () => {
    const app = new SpacetimeDBCheckboxApp();
    
    // Mock connection failure
    vi.spyOn(app['checkboxDatabase'], 'connect').mockResolvedValue(false);
    
    // Should not throw error even if auto-connect fails
    expect(() => app.initializeCanvas(canvas)).not.toThrow();
    
    // Wait for auto-connect attempt
    await new Promise(resolve => setTimeout(resolve, 100));
    
    // App should still be usable for offline mode
    expect(app.getCheckboxState(0, 0)).toBe(false);
  });
});
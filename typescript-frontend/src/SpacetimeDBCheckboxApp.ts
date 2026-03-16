import { MemoryManager, type RenderStrategy } from './MemoryManager.js';
import { ViewportManager } from './ViewportManager.js';
import { CanvasRenderer } from './CanvasRenderer.js';
import { NavigationController } from './NavigationController.js';

interface CheckboxState {
  checked: boolean;
}

export class SpacetimeDBCheckboxApp {
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private checkboxStates: Map<string, CheckboxState> = new Map();
  private chunkData: Map<number, Uint8Array> = new Map();
  
  // Large canvas components
  private memoryManager: MemoryManager;
  private viewportManager: ViewportManager | null = null;
  private canvasRenderer: CanvasRenderer | null = null;
  private navigationController: NavigationController | null = null;
  private renderStrategy: RenderStrategy;
  
  // Dynamic grid configuration based on memory capabilities
  private gridCols: number = 100; // Target: 100x100 grid
  private gridRows: number = 100;
  private canvasWidth: number = 3200; // Target: 3200x3200px
  private canvasHeight: number = 3200;
  private viewportWidth: number = 800;
  private viewportHeight: number = 600;
  
  private readonly CELL_SIZE = 32;
  private readonly SERVER_URL: string;

  constructor(serverUrl?: string) {
    // Access environment variables for Vite
    const envUrl = (import.meta as any).env?.VITE_SPACETIMEDB_URL;
    this.SERVER_URL = serverUrl || envUrl || 'http://localhost:3000';
    console.log('SpacetimeDB server URL:', this.SERVER_URL);
    this.memoryManager = new MemoryManager();
    this.renderStrategy = this.memoryManager.getFallbackStrategy();
    this.adjustConfigurationForStrategy();
  }
  
  private showConnectionError(message: string): void {
    const errorDiv = document.createElement('div');
    errorDiv.style.cssText = 'position:fixed;top:20px;right:20px;background:#ff4444;color:white;padding:15px;border-radius:5px;z-index:9999;max-width:300px;';
    errorDiv.textContent = message;
    document.body.appendChild(errorDiv);
    setTimeout(() => errorDiv.remove(), 5000);
  }

  private adjustConfigurationForStrategy(): void {
    const config = this.memoryManager.getConfigForStrategy(this.renderStrategy);
    this.gridCols = Math.min(this.gridCols, config.maxCells);
    this.gridRows = Math.min(this.gridRows, config.maxCells);
    this.canvasWidth = this.gridCols * this.CELL_SIZE;
    this.canvasHeight = this.gridRows * this.CELL_SIZE;
    
    console.log(`Adjusted grid to ${this.gridCols}×${this.gridRows} for ${this.renderStrategy} strategy`);
  }

  async initializeConnection(): Promise<boolean> {
    try {
      console.log('Demo mode: SpacetimeDB connection disabled for deployment');
      this.showConnectionInfo('Running in demo mode - changes not synchronized');
      return true;
    } catch (error) {
      console.error('Connection initialization failed:', error);
      this.showConnectionError('Unable to connect to SpacetimeDB server. Running in offline mode.');
      return false;
    }
  }

  private showConnectionInfo(message: string): void {
    const infoDiv = document.createElement('div');
    infoDiv.style.cssText = 'position:fixed;top:20px;left:20px;background:#2196F3;color:white;padding:15px;border-radius:5px;z-index:9999;max-width:300px;';
    infoDiv.textContent = message;
    document.body.appendChild(infoDiv);
    setTimeout(() => infoDiv.remove(), 3000);
  }

  private subscribeToChunkUpdates(): void {
    console.log('Subscription setup skipped in demo mode');
  }

  private onInsert(ctx: any, row: any): void {
    // Handle new chunk data
    this.processChunkUpdate(row);
  }

  private onUpdate(ctx: any, oldRow: any, newRow: any): void {
    // Handle chunk updates
    this.processChunkUpdate(newRow);
  }

  private processChunkUpdate(chunk: any): void {
    if (!chunk || typeof chunk.chunk_id !== 'number') return;
    
    try {
      const chunkId = chunk.chunk_id;
      const state = chunk.state instanceof Uint8Array ? chunk.state : new Uint8Array(chunk.state);
      
      this.chunkData.set(chunkId, state);
      this.markChunkForRerender(chunkId);
    } catch (error) {
      console.error('Error processing chunk update:', error);
    }
  }

  private markChunkForRerender(chunkId: number): void {
    if (!this.canvasRenderer) return;
    
    const chunkSize = 1000000; // 1M checkboxes per chunk
    const startRow = Math.floor(chunkId * chunkSize / this.gridCols);
    const endRow = Math.floor((chunkId + 1) * chunkSize / this.gridCols);
    
    for (let row = startRow; row < endRow && row < this.gridRows; row++) {
      this.canvasRenderer.markRowForRerender(row);
    }
  }

  private getCheckboxState(row: number, col: number): boolean {
    const globalIndex = row * this.gridCols + col;
    const chunkId = Math.floor(globalIndex / 1000000); // 1M checkboxes per chunk
    const localIndex = globalIndex % 1000000;
    
    const chunk = this.chunkData.get(chunkId);
    if (!chunk) return false;
    
    const byteIndex = Math.floor(localIndex / 8);
    const bitIndex = localIndex % 8;
    
    if (byteIndex >= chunk.length) return false;
    
    return (chunk[byteIndex] & (1 << bitIndex)) !== 0;
  }

  private async setCheckboxState(row: number, col: number, checked: boolean): Promise<void> {
    try {
      const globalIndex = row * this.gridCols + col;
      const chunkId = Math.floor(globalIndex / 1000000);
      const localIndex = globalIndex % 1000000;
      
      // Update local state immediately for responsiveness
      let chunk = this.chunkData.get(chunkId);
      if (!chunk) {
        chunk = new Uint8Array(125000); // 125KB per chunk
        this.chunkData.set(chunkId, chunk);
      }
      
      const byteIndex = Math.floor(localIndex / 8);
      const bitIndex = localIndex % 8;
      
      if (byteIndex < chunk.length) {
        if (checked) {
          chunk[byteIndex] |= (1 << bitIndex);
        } else {
          chunk[byteIndex] &= ~(1 << bitIndex);
        }
        
        // Mark for re-render
        this.markChunkForRerender(chunkId);
      }
      
      console.log(`Demo mode: Checkbox at ${row},${col} set to ${checked} (local only)`);
      
    } catch (error) {
      console.error('Error setting checkbox state:', error);
    }
  }

  async initialize(): Promise<boolean> {
    console.log('Initializing SpacetimeDB Checkbox App...');
    
    // Initialize connection
    const connected = await this.initializeConnection();
    
    // Set up canvas
    this.setupCanvas();
    
    // Initialize components
    this.initializeComponents();
    
    console.log('SpacetimeDB Checkbox App initialization complete');
    return connected;
  }

  private setupCanvas(): void {
    const container = document.getElementById('viewport-container');
    if (!container) {
      throw new Error('Container element not found');
    }
    
    // Apply container styles
    container.style.cssText = `
      position: relative;
      width: ${this.viewportWidth}px;
      height: ${this.viewportHeight}px;
      border: 2px solid #333;
      overflow: hidden;
      background: #f0f0f0;
      margin: 20px auto;
      border-radius: 8px;
      box-shadow: 0 4px 8px rgba(0,0,0,0.1);
    `;
    
    this.canvas = document.createElement('canvas');
    this.canvas.width = this.canvasWidth;
    this.canvas.height = this.canvasHeight;
    this.canvas.style.cssText = `
      position: absolute;
      top: 0;
      left: 0;
      cursor: pointer;
      image-rendering: pixelated;
      image-rendering: -moz-crisp-edges;
    `;
    
    container.appendChild(this.canvas);
    
    this.ctx = this.canvas.getContext('2d');
    if (!this.ctx) {
      throw new Error('Could not get canvas context');
    }
  }

  private initializeComponents(): void {
    if (!this.canvas || !this.ctx) return;
    
    this.viewportManager = new ViewportManager(
      this.canvasWidth,
      this.canvasHeight,
      this.viewportWidth,
      this.viewportHeight
    );
    
    this.canvasRenderer = new CanvasRenderer(
      this.ctx,
      this.gridCols,
      this.gridRows,
      this.CELL_SIZE,
      this.renderStrategy,
      (row: number, col: number) => this.getCheckboxState(row, col)
    );
    
    this.navigationController = new NavigationController(
      this.viewportManager,
      (x: number, y: number) => this.handleCanvasClick(x, y)
    );
    
    // Set up event listeners
    this.setupEventListeners();
    
    // Initial render
    this.renderLoop();
  }

  private setupEventListeners(): void {
    if (!this.canvas || !this.navigationController) return;
    
    // Mouse events for clicking and navigation
    this.canvas.addEventListener('click', (e) => {
      this.navigationController?.handleClick(e);
    });
    
    // Keyboard events for navigation
    document.addEventListener('keydown', (e) => {
      if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space'].includes(e.key)) {
        e.preventDefault();
        this.navigationController?.handleKeydown(e);
      }
    });
  }

  private handleCanvasClick(canvasX: number, canvasY: number): void {
    const col = Math.floor(canvasX / this.CELL_SIZE);
    const row = Math.floor(canvasY / this.CELL_SIZE);
    
    if (row >= 0 && row < this.gridRows && col >= 0 && col < this.gridCols) {
      const currentState = this.getCheckboxState(row, col);
      this.setCheckboxState(row, col, !currentState);
    }
  }

  private renderLoop(): void {
    if (!this.canvasRenderer || !this.viewportManager) return;
    
    const viewport = this.viewportManager.getViewport();
    this.canvasRenderer.render(viewport);
    
    // Update canvas transform for panning
    if (this.canvas) {
      this.canvas.style.transform = `translate(${-viewport.x}px, ${-viewport.y}px)`;
    }
    
    // Continue render loop
    requestAnimationFrame(() => this.renderLoop());
  }

  // Public API
  public getGridSize(): { rows: number; cols: number } {
    return { rows: this.gridRows, cols: this.gridCols };
  }

  public getViewport() {
    return this.viewportManager?.getViewport();
  }

  public panTo(x: number, y: number): void {
    this.viewportManager?.panTo(x, y);
  }
}
import { CheckboxDatabase, CheckboxChunk } from './generated/CheckboxDatabase.js';

interface CheckboxState {
  checked: boolean;
}

export class SpacetimeDBCheckboxApp {
  private canvas: HTMLCanvasElement | null = null;
  private ctx: CanvasRenderingContext2D | null = null;
  private container: HTMLElement | null = null;
  
  // Grid configuration
  private readonly gridSize = 100;
  private readonly cellSize = 32;
  private readonly viewportWidth = 800;
  private readonly viewportHeight = 600;
  private readonly viewportCols = Math.floor(800 / 32); // 25 cols
  private readonly viewportRows = Math.floor(600 / 32); // 18 rows
  
  // Navigation state
  private currentX = 0;
  private currentY = 0;
  private viewportX = 0;
  private viewportY = 0;
  
  // Checkbox state (cached from SpacetimeDB)
  private chunkData: Map<number, Uint8Array> = new Map();
  private checkedCount = 0;
  
  // Configuration
  private serverUrl: string;
  private databaseAddress: string;
  
  // Database client
  private checkboxDatabase: CheckboxDatabase;

  constructor(serverUrl: string = 'http://localhost:3000', databaseAddress: string = 'checkboxes-local-demo') {
    this.serverUrl = serverUrl;
    this.databaseAddress = databaseAddress;
    this.checkboxDatabase = new CheckboxDatabase(serverUrl, databaseAddress);
    console.log(`SpacetimeDB Checkbox App initialized with ${serverUrl} / ${databaseAddress}`);
  }
  
  // Initialize canvas manually (called from HTML)
  public initializeCanvas(canvas: HTMLCanvasElement): void {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    
    if (!this.ctx) {
      throw new Error('Failed to get 2D context from canvas');
    }
    
    // Set canvas size
    this.canvas.width = this.gridSize * this.cellSize;
    this.canvas.height = this.gridSize * this.cellSize;
    
    // Set canvas viewport styling
    this.canvas.style.transform = 'translate(0px, 0px)';
    
    // Set up event listeners for navigation
    this.setupEventListeners();
    
    // Initial render
    this.render();
    
    console.log('Canvas initialized successfully');
  }
  
  // Public connect method (called from HTML)
  public async connect(): Promise<boolean> {
    console.log('Connecting to SpacetimeDB...');
    
    try {
      // Connect to SpacetimeDB
      const connected = await this.checkboxDatabase.connect();
      if (!connected) {
        console.error('Failed to connect to SpacetimeDB');
        return false;
      }
      
      // Subscribe to database updates
      this.subscribeToUpdates();
      
      // Load initial state
      await this.loadInitialState();
      
      console.log('Connected to SpacetimeDB successfully');
      return true;
    } catch (error) {
      console.error('Failed to connect to SpacetimeDB:', error);
      return false;
    }
  }
  
  // Public method to toggle a checkbox (called from HTML)
  public async toggleCheckbox(x: number, y: number): Promise<void> {
    await this.toggleCheckboxInternal(y, x); // Note: internal method expects row, col
  }
  
  // Get checkbox statistics (called from HTML)
  public getCheckboxCount(): { total: number; checked: number } {
    return { total: this.gridSize * this.gridSize, checked: this.checkedCount };
  }
  
  // Set up canvas and container
  private setupCanvas(): void {
    this.container = document.getElementById('grid-container');
    if (!this.container) {
      throw new Error('Container element #grid-container not found');
    }
    
    // Clear and set up container
    this.container.innerHTML = '';
    this.container.style.cssText = `
      position: relative;
      width: ${this.viewportWidth}px;
      height: ${this.viewportHeight}px;
      border: 2px solid #333;
      overflow: hidden;
      margin: 20px auto;
      border-radius: 8px;
      box-shadow: 0 4px 8px rgba(0,0,0,0.1);
      background: white;
    `;
    
    // Create canvas
    this.canvas = document.createElement('canvas');
    this.canvas.width = this.gridSize * this.cellSize;
    this.canvas.height = this.gridSize * this.cellSize;
    this.canvas.style.cssText = `
      position: absolute;
      top: 0;
      left: 0;
      cursor: pointer;
      transition: transform 0.1s ease-out;
    `;
    
    this.container.appendChild(this.canvas);
    
    this.ctx = this.canvas.getContext('2d');
    if (!this.ctx) {
      throw new Error('Could not get canvas context');
    }
  }
  
  // Set up event listeners
  private setupEventListeners(): void {
    if (!this.canvas) return;
    
    // Canvas click events
    this.canvas.addEventListener('click', (e) => {
      const rect = this.canvas!.getBoundingClientRect();
      const x = e.clientX - rect.left + this.viewportX;
      const y = e.clientY - rect.top + this.viewportY;
      const col = Math.floor(x / this.cellSize);
      const row = Math.floor(y / this.cellSize);
      
      if (row >= 0 && row < this.gridSize && col >= 0 && col < this.gridSize) {
        this.toggleCheckboxInternal(row, col);
      }
    });
    
    // Keyboard navigation
    document.addEventListener('keydown', (e) => {
      let moved = false;
      
      switch (e.key) {
        case 'ArrowUp':
          if (this.currentY > 0) {
            this.currentY--;
            moved = true;
          }
          break;
        case 'ArrowDown':
          if (this.currentY < this.gridSize - 1) {
            this.currentY++;
            moved = true;
          }
          break;
        case 'ArrowLeft':
          if (this.currentX > 0) {
            this.currentX--;
            moved = true;
          }
          break;
        case 'ArrowRight':
          if (this.currentX < this.gridSize - 1) {
            this.currentX++;
            moved = true;
          }
          break;
        case ' ':
          e.preventDefault();
          this.toggleCheckboxInternal(this.currentY, this.currentX);
          return;
      }
      
      if (moved) {
        e.preventDefault();
        this.updateViewport();
        this.updateStats();
      }
    });
  }
  
  // Subscribe to SpacetimeDB updates
  private subscribeToUpdates(): void {
    this.checkboxDatabase.onCheckboxChunkInsert((chunk: CheckboxChunk) => {
      console.log('Chunk inserted:', chunk.chunkId);
      this.chunkData.set(chunk.chunkId, new Uint8Array(chunk.state));
      this.render();
    });
    
    this.checkboxDatabase.onCheckboxChunkUpdate((newRow: CheckboxChunk) => {
      console.log('Chunk updated:', newRow.chunkId);
      this.chunkData.set(newRow.chunkId, new Uint8Array(newRow.state));
      this.render();
    });
  }
  
  // Load initial state from SpacetimeDB  
  public async loadInitialState(): Promise<void> {
    try {
      const chunks = await this.checkboxDatabase.getAllChunks();
      console.log(`Loaded ${chunks.length} chunks from SpacetimeDB`);
      
      for (const chunk of chunks) {
        this.chunkData.set(chunk.chunkId, new Uint8Array(chunk.state));
      }
      
      this.render();
      this.updateStats();
    } catch (error) {
      console.error('Failed to load initial state:', error);
    }
  }
  
  // Toggle a checkbox and sync to SpacetimeDB
  private async toggleCheckboxInternal(row: number, col: number): Promise<void> {
    const globalIndex = row * this.gridSize + col;
    const chunkId = Math.floor(globalIndex / 1000000); // 1M checkboxes per chunk
    const bitOffset = globalIndex % 1000000;
    
    // Get current state
    const currentState = this.getCheckboxState(row, col);
    const newState = !currentState;
    
    try {
      // Update SpacetimeDB first
      await this.checkboxDatabase.updateCheckbox(chunkId, bitOffset, newState);
      
      // Update local cache immediately for responsiveness
      let chunk = this.chunkData.get(chunkId);
      if (!chunk) {
        chunk = new Uint8Array(125000); // 125KB for 1M bits
        this.chunkData.set(chunkId, chunk);
        
        // Create chunk in database if it doesn't exist
        await this.checkboxDatabase.addChunk(chunkId);
      }
      
      const byteIndex = Math.floor(bitOffset / 8);
      const bitIndex = bitOffset % 8;
      
      if (byteIndex < chunk.length) {
        if (newState) {
          chunk[byteIndex] |= (1 << bitIndex);
        } else {
          chunk[byteIndex] &= ~(1 << bitIndex);
        }
      }
      
      this.render();
      this.updateStats();
      
      console.log(`Toggled checkbox (${row}, ${col}) to ${newState}`);
      
    } catch (error) {
      console.error('Failed to toggle checkbox:', error);
      this.showStatus(`Failed to update: ${error.message}`, 'error');
      setTimeout(() => this.hideStatus(), 3000);
    }
  }
  
  // Get checkbox state from chunk data
  private getCheckboxState(row: number, col: number): boolean {
    const globalIndex = row * this.gridSize + col;
    const chunkId = Math.floor(globalIndex / 1000000);
    const bitOffset = globalIndex % 1000000;
    
    const chunk = this.chunkData.get(chunkId);
    if (!chunk) return false;
    
    const byteIndex = Math.floor(bitOffset / 8);
    const bitIndex = bitOffset % 8;
    
    if (byteIndex >= chunk.length) return false;
    
    return (chunk[byteIndex] & (1 << bitIndex)) !== 0;
  }
  
  // Update viewport to follow current position
  private updateViewport(): void {
    const centerX = this.currentX * this.cellSize;
    const centerY = this.currentY * this.cellSize;
    
    // Calculate desired viewport position to center current position
    this.viewportX = centerX - this.viewportWidth / 2;
    this.viewportY = centerY - this.viewportHeight / 2;
    
    // Clamp to grid boundaries
    this.viewportX = Math.max(0, Math.min(this.viewportX, this.gridSize * this.cellSize - this.viewportWidth));
    this.viewportY = Math.max(0, Math.min(this.viewportY, this.gridSize * this.cellSize - this.viewportHeight));
    
    // Update canvas transform
    if (this.canvas) {
      this.canvas.style.transform = `translate(${-this.viewportX}px, ${-this.viewportY}px)`;
    }
  }
  
  // Render the grid
  private render(): void {
    if (!this.ctx) return;
    
    const ctx = this.ctx;
    
    // Clear canvas
    ctx.clearRect(0, 0, this.gridSize * this.cellSize, this.gridSize * this.cellSize);
    
    // Draw grid and checkboxes
    for (let row = 0; row < this.gridSize; row++) {
      for (let col = 0; col < this.gridSize; col++) {
        const x = col * this.cellSize;
        const y = row * this.cellSize;
        const isChecked = this.getCheckboxState(row, col);
        const isCurrent = row === this.currentY && col === this.currentX;
        
        // Draw cell background
        ctx.fillStyle = isChecked ? '#4CAF50' : 'white';
        ctx.fillRect(x, y, this.cellSize, this.cellSize);
        
        // Draw cell border
        ctx.strokeStyle = isCurrent ? '#2196F3' : '#ccc';
        ctx.lineWidth = isCurrent ? 3 : 1;
        ctx.strokeRect(x, y, this.cellSize, this.cellSize);
        
        // Draw checkmark
        if (isChecked) {
          ctx.fillStyle = 'white';
          ctx.font = '20px Arial';
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          ctx.fillText('✓', x + this.cellSize / 2, y + this.cellSize / 2);
        }
      }
    }
  }
  
  // Start render loop
  private renderLoop(): void {
    this.render();
    requestAnimationFrame(() => this.renderLoop());
  }
  
  // Update statistics display
  private updateStats(): void {
    let checkedCount = 0;
    
    // Count checked checkboxes
    for (const chunk of this.chunkData.values()) {
      for (const byte of chunk) {
        for (let bit = 0; bit < 8; bit++) {
          if (byte & (1 << bit)) {
            checkedCount++;
          }
        }
      }
    }
    
    const viewportStartCol = Math.floor(this.viewportX / this.cellSize);
    const viewportEndCol = Math.min(this.gridSize - 1, viewportStartCol + this.viewportCols - 1);
    const viewportStartRow = Math.floor(this.viewportY / this.cellSize);
    const viewportEndRow = Math.min(this.gridSize - 1, viewportStartRow + this.viewportRows - 1);
    
    const positionInfo = document.getElementById('viewportPosition');
    const checkedCountEl = document.getElementById('checkedCount');
    const viewportInfo = document.getElementById('viewport-info');
    
    if (positionInfo) positionInfo.textContent = `${this.currentX}, ${this.currentY}`;
    if (checkedCountEl) checkedCountEl.textContent = `${checkedCount}`;
    if (viewportInfo) viewportInfo.textContent = `Viewport: ${viewportStartCol}-${viewportEndCol} × ${viewportStartRow}-${viewportEndRow}`;
    
    // Store checked count for external access
    this.checkedCount = checkedCount;
  }
  
  // Show status message
  private showStatus(message: string, type: 'info' | 'error' = 'info'): void {
    let status = document.querySelector('.status') as HTMLElement;
    if (!status) {
      status = document.createElement('div');
      status.className = 'status';
      status.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        padding: 10px 15px;
        border-radius: 5px;
        color: white;
        font-weight: bold;
        z-index: 1000;
      `;
      document.body.appendChild(status);
    }
    
    status.textContent = message;
    status.style.background = type === 'info' ? '#2196F3' : '#f44336';
    status.style.display = 'block';
  }
  
  // Hide status message
  private hideStatus(): void {
    const status = document.querySelector('.status') as HTMLElement;
    if (status) {
      status.style.display = 'none';
    }
  }
  
  // Public API
  public isConnected(): boolean {
    return this.checkboxDatabase.isConnected();
  }
  
  public getGridSize(): { rows: number; cols: number } {
    return { rows: this.gridSize, cols: this.gridSize };
  }
}
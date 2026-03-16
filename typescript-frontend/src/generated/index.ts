// SpacetimeDB client for checkbox grid - using SDK exports
import { 
  DbConnectionBuilder,
  DbConnectionImpl,
} from "spacetimedb";

export interface CheckboxChunk {
  chunk_id: number;
  state: Uint8Array;
  version: number;
}

// Default database connection configuration (can be overridden in constructor)
const DEFAULT_DATABASE_ADDRESS = 'c200d12d98ef0c856a8ba926a0f711a75ef243fe097a24f6c26836f0ff2215a0';
const DEFAULT_SERVER_URL = 'https://maincloud.spacetimedb.com';

// Simple remote module for our database
const REMOTE_MODULE = {
  procedures: [],
  tables: {},
  reducers: [],
  versionInfo: {
    cliVersion: "2.0.0"
  }
} as const;

// Database client class
export class CheckboxDatabase {
  private connection: DbConnectionImpl<typeof REMOTE_MODULE> | null = null;
  private connected: boolean = false;
  private chunks = new Map<number, CheckboxChunk>();
  
  // Configuration parameters
  private serverUrl: string;
  private databaseAddress: string;
  
  constructor(serverUrl: string = DEFAULT_SERVER_URL, databaseAddress: string = DEFAULT_DATABASE_ADDRESS) {
    this.serverUrl = serverUrl;
    this.databaseAddress = databaseAddress;
  }
  
  // Connect to the database
  async connect(): Promise<boolean> {
    try {
      console.log('🔌 Connecting to SpacetimeDB at:', this.serverUrl);
      console.log('📍 Database address:', this.databaseAddress);
      
      // Create connection using builder pattern
      const builder = new DbConnectionBuilder(REMOTE_MODULE, (config: any) => new DbConnectionImpl(config));
      this.connection = builder
        .withUri(this.serverUrl)
        .withDatabaseName(this.databaseAddress)
        .onConnect((connection: any, identity: any, token: string) => {
          console.log('✅ Connected with identity:', identity?.toHexString?.() || identity);
          this.connected = true;
          this.setupSubscriptions();
        })
        .onConnectError((ctx: any, error: Error) => {
          console.error('❌ Connection error:', error);
          this.connected = false;
        })
        .build();
        
      // Wait for connection to be established
      return new Promise<boolean>((resolve) => {
        let attempts = 0;
        const checkConnection = () => {
          attempts++;
          if (this.connected) {
            console.log('✅ Successfully connected to SpacetimeDB!');
            resolve(true);
          } else if (attempts > 15) {
            console.log('❌ Connection timeout after 3 seconds');
            resolve(false);
          } else {
            setTimeout(checkConnection, 200);
          }
        };
        checkConnection();
      });
      
    } catch (error) {
      console.error('❌ Failed to connect to SpacetimeDB:', error);
      this.connected = false;
      return false;
    }
  }
  
  // Set up real-time subscriptions
  private setupSubscriptions(): void {
    if (!this.connection) return;
    
    try {
      console.log('📡 Setting up subscriptions for real-time updates...');
      
      // Subscribe to all checkbox chunk updates
      this.connection.subscriptionBuilder()
        .onTable('CheckboxChunk', 
          (chunk: CheckboxChunk, operation: string) => {
            console.log(`Chunk ${operation}:`, chunk.chunk_id);
            this.chunks.set(chunk.chunk_id, {
              ...chunk,
              state: new Uint8Array(chunk.state)
            });
            
            if (operation === 'insert') {
              this.notifyChunkInsert(chunk);
            } else if (operation === 'update') {
              this.notifyChunkUpdate(chunk, chunk);
            }
          })
        .subscribe();
        
      console.log('✅ Real-time subscriptions active');
    } catch (error) {
      console.error('❌ Failed to set up subscriptions:', error);
      // Continue without subscriptions - we'll still have manual refresh
    }
  }
  
  // Callbacks for subscription events
  private insertCallbacks: ((chunk: CheckboxChunk) => void)[] = [];
  private updateCallbacks: ((oldRow: CheckboxChunk, newRow: CheckboxChunk) => void)[] = [];
  
  private notifyChunkInsert(chunk: CheckboxChunk): void {
    this.insertCallbacks.forEach(cb => cb(chunk));
  }
  
  private notifyChunkUpdate(oldChunk: CheckboxChunk, newChunk: CheckboxChunk): void {
    this.updateCallbacks.forEach(cb => cb(oldChunk, newChunk));
  }
  
  
  // Check connection status
  isConnected(): boolean {
    return this.connected && this.connection !== null;
  }
  
  // Subscribe to table updates
  onCheckboxChunkInsert(callback: (chunk: CheckboxChunk) => void): void {
    this.insertCallbacks.push(callback);
  }
  
  onCheckboxChunkUpdate(callback: (oldRow: CheckboxChunk, newRow: CheckboxChunk) => void): void {
    this.updateCallbacks.push(callback);
  }
  
  // Call reducers
  async updateCheckbox(chunk_id: number, bit_offset: number, checked: boolean): Promise<void> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      // Call the update_checkbox reducer with proper parameters
      await this.connection.call('update_checkbox', [chunk_id, bit_offset, checked]);
      console.log(`✅ Updated checkbox ${chunk_id}:${bit_offset} = ${checked}`);
    } catch (error) {
      console.error('❌ Failed to update checkbox:', error);
      throw error;
    }
  }
  
  async addChunk(chunk_id: number): Promise<void> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      // Call the add_chunk reducer  
      await this.connection.call('add_chunk', [chunk_id]);
      console.log(`✅ Added chunk ${chunk_id}`);
    } catch (error) {
      console.error('❌ Failed to add chunk:', error);
      throw error;
    }
  }
  
  // Query all chunks
  async getAllChunks(): Promise<CheckboxChunk[]> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      // Query all CheckboxChunk records
      const queryResult = await this.connection.query('SELECT * FROM CheckboxChunk');
      console.log(`📦 Loaded ${queryResult.length} chunks from SpacetimeDB`);
      
      // Convert the results to our format and cache them
      const chunks = queryResult.map((row: any) => ({
        chunk_id: row.chunk_id,
        state: new Uint8Array(row.state),
        version: row.version
      }));
      
      // Update local cache
      chunks.forEach(chunk => this.chunks.set(chunk.chunk_id, chunk));
      
      return chunks;
    } catch (error) {
      console.error('❌ Failed to get chunks:', error);
      
      // Fallback: return cached chunks if query fails
      console.log('📦 Returning cached chunks as fallback');
      return Array.from(this.chunks.values());
    }
  }
  
  // Query chunk by ID
  async getChunkById(chunk_id: number): Promise<CheckboxChunk | null> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      // First check cache
      if (this.chunks.has(chunk_id)) {
        return this.chunks.get(chunk_id)!;
      }
      
      // Query specific chunk from database
      const queryResult = await this.connection.query(
        `SELECT * FROM CheckboxChunk WHERE chunk_id = ${chunk_id}`
      );
      
      if (queryResult.length > 0) {
        const chunk = {
          chunk_id: queryResult[0].chunk_id,
          state: new Uint8Array(queryResult[0].state),
          version: queryResult[0].version
        };
        this.chunks.set(chunk_id, chunk);
        return chunk;
      }
      
      return null;
    } catch (error) {
      console.error('❌ Failed to get chunk:', error);
      
      // Fallback: check cache
      return this.chunks.get(chunk_id) || null;
    }
  }
  
  // Disconnect from database
  async disconnect(): Promise<void> {
    if (this.connection) {
      try {
        await this.connection.close();
        console.log('🔌 Disconnected from SpacetimeDB');
      } catch (error) {
        console.error('Error disconnecting:', error);
      }
      this.connection = null;
    }
    this.connected = false;
    this.chunks.clear();
  }
}
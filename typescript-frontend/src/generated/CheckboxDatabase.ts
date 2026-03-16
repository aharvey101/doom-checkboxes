// SpacetimeDB client for checkbox grid - using proper v2.0 SDK
import { 
  DbConnection,
  tables,
  reducers,
  type SubscriptionHandle,
} from './index';

// Default database connection configuration (can be overridden in constructor)
const DEFAULT_DATABASE_ADDRESS = 'c200d12d98ef0c856a8ba926a0f711a75ef243fe097a24f6c26836f0ff2215a0';
const DEFAULT_SERVER_URL = 'https://maincloud.spacetimedb.com';

// Define the checkbox chunk type from our generated table
export type CheckboxChunk = {
  chunkId: number;
  state: Uint8Array;
  version: number;
};

// Database client class using proper SpacetimeDB v2.0 API
export class CheckboxDatabase {
  private connection: DbConnection | null = null;
  private connected: boolean = false;
  private subscriptionHandle: SubscriptionHandle | null = null;
  
  // Configuration parameters
  private serverUrl: string;
  private databaseAddress: string;
  
  // Callbacks for subscription events
  private insertCallbacks: ((chunk: CheckboxChunk) => void)[] = [];
  private updateCallbacks: ((chunk: CheckboxChunk) => void)[] = [];
  
  constructor(serverUrl: string = DEFAULT_SERVER_URL, databaseAddress: string = DEFAULT_DATABASE_ADDRESS) {
    this.serverUrl = serverUrl;
    this.databaseAddress = databaseAddress;
  }
  
  // Connect to the database
  async connect(): Promise<boolean> {
    try {
      console.log('🔌 Connecting to SpacetimeDB at:', this.serverUrl);
      console.log('📍 Database address:', this.databaseAddress);
      
      // Create connection using v2.0 API
      this.connection = DbConnection.builder()
        .withUri(this.serverUrl)
        .withDatabaseName(this.databaseAddress)
        .onConnect((connection, identity, token) => {
          console.log('✅ Connected with identity:', identity?.toHexString?.() || identity);
          this.connected = true;
          this.setupSubscriptions();
        })
        .onConnectError((ctx, error) => {
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
  
  // Set up real-time subscriptions using v2.0 API
  private setupSubscriptions(): void {
    if (!this.connection) return;
    
    try {
      console.log('📡 Setting up subscriptions for real-time updates...');
      
      // Subscribe to the checkbox_chunk table using v2.0 API
      this.subscriptionHandle = this.connection.subscriptionBuilder()
        .subscribe(tables.checkbox_chunk);
        
      // Set up insert and update callbacks
      this.connection.db.checkbox_chunk.onInsert((ctx, chunk, reducerEvent) => {
        console.log('Chunk insert:', chunk.chunkId);
        const convertedChunk: CheckboxChunk = {
          chunkId: chunk.chunkId,
          state: new Uint8Array(chunk.state),
          version: chunk.version
        };
        this.notifyChunkInsert(convertedChunk);
      });
      
      this.connection.db.checkbox_chunk.onUpdate((ctx, oldChunk, newChunk, reducerEvent) => {
        console.log('Chunk update:', newChunk.chunkId);
        const convertedChunk: CheckboxChunk = {
          chunkId: newChunk.chunkId,
          state: new Uint8Array(newChunk.state),
          version: newChunk.version
        };
        this.notifyChunkUpdate(convertedChunk);
      });
        
      console.log('✅ Real-time subscriptions active');
    } catch (error) {
      console.error('❌ Failed to set up subscriptions:', error);
      // Continue without subscriptions - we'll still have manual refresh
    }
  }
  
  private notifyChunkInsert(chunk: CheckboxChunk): void {
    this.insertCallbacks.forEach(cb => cb(chunk));
  }
  
  private notifyChunkUpdate(chunk: CheckboxChunk): void {
    this.updateCallbacks.forEach(cb => cb(chunk));
  }
  
  // Check connection status
  isConnected(): boolean {
    return this.connected && this.connection !== null;
  }
  
  // Subscribe to table updates
  onCheckboxChunkInsert(callback: (chunk: CheckboxChunk) => void): void {
    this.insertCallbacks.push(callback);
  }
  
  onCheckboxChunkUpdate(callback: (chunk: CheckboxChunk) => void): void {
    this.updateCallbacks.push(callback);
  }
  
  // Call reducers using v2.0 API
  async updateCheckbox(chunk_id: number, bit_offset: number, checked: boolean): Promise<void> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      // Call the update_checkbox reducer using v2.0 API
      await this.connection.reducers.updateCheckbox(chunk_id, bit_offset, checked);
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
      // Call the add_chunk reducer using v2.0 API
      await this.connection.reducers.addChunk(chunk_id);
      console.log(`✅ Added chunk ${chunk_id}`);
    } catch (error) {
      console.error('❌ Failed to add chunk:', error);
      throw error;
    }
  }
  
  // Query all chunks using v2.0 API
  async getAllChunks(): Promise<CheckboxChunk[]> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      // Query all CheckboxChunk records using v2.0 API
      const chunks: CheckboxChunk[] = [];
      
      // Use the table query API to get all rows
      for (const row of this.connection.db.checkbox_chunk.iter()) {
        chunks.push({
          chunkId: row.chunkId,
          state: new Uint8Array(row.state),
          version: row.version
        });
      }
      
      console.log(`📦 Loaded ${chunks.length} chunks from SpacetimeDB`);
      return chunks;
    } catch (error) {
      console.error('❌ Failed to get chunks:', error);
      // Return empty array as fallback
      return [];
    }
  }
  
  // Query chunk by ID using v2.0 API
  async getChunkById(chunk_id: number): Promise<CheckboxChunk | null> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      // Query specific chunk using unique column accessor
      const row = this.connection.db.checkbox_chunk.chunkId.find(chunk_id);
      
      if (row) {
        return {
          chunkId: row.chunkId,
          state: new Uint8Array(row.state),
          version: row.version
        };
      }
      
      return null;
    } catch (error) {
      console.error('❌ Failed to get chunk:', error);
      return null;
    }
  }
  
  // Disconnect from database
  async disconnect(): Promise<void> {
    if (this.subscriptionHandle) {
      this.subscriptionHandle.unsubscribe();
      this.subscriptionHandle = null;
    }
    
    if (this.connection) {
      try {
        this.connection.disconnect();
        console.log('🔌 Disconnected from SpacetimeDB');
      } catch (error) {
        console.error('Error disconnecting:', error);
      }
      this.connection = null;
    }
    this.connected = false;
  }
}
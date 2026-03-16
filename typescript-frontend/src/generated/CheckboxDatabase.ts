// SpacetimeDB client for checkbox grid - using proper v2.0 SDK
import { 
  DbConnection,
  tables,
  reducers,
  type SubscriptionHandle,
} from './index';

// Default database connection configuration (can be overridden in constructor)
const DEFAULT_DATABASE_ADDRESS = 'c200f55a1042dfa3abafc82273cc1d9daa4268eb87ec63ed798f504971ff6754';
const DEFAULT_SERVER_URL = 'http://localhost:3001';

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
      console.log('📡 [DEBUG-SUB] Setting up subscriptions for real-time updates...');
      
      // Subscribe to the checkbox_chunk table using v2.0 API
      console.log('📡 [DEBUG-SUB] Creating subscription builder...');
      this.subscriptionHandle = this.connection.subscriptionBuilder()
        .subscribe(tables.checkbox_chunk);
      console.log('📡 [DEBUG-SUB] Subscription handle created:', this.subscriptionHandle ? 'success' : 'failed');
        
      // Set up insert and update callbacks
      console.log('📡 [DEBUG-SUB] Setting up onInsert callback...');
      this.connection.db.checkbox_chunk.onInsert((ctx, chunk, reducerEvent) => {
        console.log('🔔 [DEBUG-INSERT] Insert callback triggered!');
        console.log('🔔 [DEBUG-INSERT] Context:', ctx);
        console.log('🔔 [DEBUG-INSERT] Chunk:', chunk);
        console.log('🔔 [DEBUG-INSERT] ReducerEvent:', reducerEvent);
        console.log('🔔 [DEBUG-INSERT] Chunk insert:', chunk.chunkId);
        const convertedChunk: CheckboxChunk = {
          chunkId: chunk.chunkId,
          state: new Uint8Array(chunk.state),
          version: chunk.version
        };
        console.log('🔔 [DEBUG-INSERT] Converted chunk:', convertedChunk);
        this.notifyChunkInsert(convertedChunk);
      });
      
      console.log('📡 [DEBUG-SUB] Setting up onUpdate callback...');
      this.connection.db.checkbox_chunk.onUpdate((ctx, oldChunk, newChunk, reducerEvent) => {
        console.log('🔔 [DEBUG-UPDATE] Update callback triggered!');
        console.log('🔔 [DEBUG-UPDATE] Context:', ctx);
        console.log('🔔 [DEBUG-UPDATE] OldChunk:', oldChunk);
        console.log('🔔 [DEBUG-UPDATE] NewChunk:', newChunk);
        console.log('🔔 [DEBUG-UPDATE] ReducerEvent:', reducerEvent);
        console.log('🔔 [DEBUG-UPDATE] Chunk update:', newChunk.chunkId);
        const convertedChunk: CheckboxChunk = {
          chunkId: newChunk.chunkId,
          state: new Uint8Array(newChunk.state),
          version: newChunk.version
        };
        console.log('🔔 [DEBUG-UPDATE] Converted chunk:', convertedChunk);
        this.notifyChunkUpdate(convertedChunk);
      });
        
      console.log('✅ [DEBUG-SUB] Real-time subscriptions active');
    } catch (error) {
      console.error('❌ [DEBUG-SUB] Failed to set up subscriptions:', error);
      console.error('❌ [DEBUG-SUB] Error details:', error);
      // Continue without subscriptions - we'll still have manual refresh
    }
  }
  
  private notifyChunkInsert(chunk: CheckboxChunk): void {
    console.log(`🔔 [DEBUG-NOTIFY] notifyChunkInsert called with chunk:`, chunk);
    console.log(`🔔 [DEBUG-NOTIFY] Number of insert callbacks registered: ${this.insertCallbacks.length}`);
    
    this.insertCallbacks.forEach((cb, index) => {
      console.log(`🔔 [DEBUG-NOTIFY] Calling insert callback #${index}...`);
      try {
        cb(chunk);
        console.log(`✅ [DEBUG-NOTIFY] Insert callback #${index} completed successfully`);
      } catch (error) {
        console.error(`❌ [DEBUG-NOTIFY] Insert callback #${index} failed:`, error);
      }
    });
  }
  
  private notifyChunkUpdate(chunk: CheckboxChunk): void {
    console.log(`🔔 [DEBUG-NOTIFY] notifyChunkUpdate called with chunk:`, chunk);
    console.log(`🔔 [DEBUG-NOTIFY] Number of update callbacks registered: ${this.updateCallbacks.length}`);
    
    this.updateCallbacks.forEach((cb, index) => {
      console.log(`🔔 [DEBUG-NOTIFY] Calling update callback #${index}...`);
      try {
        cb(chunk);
        console.log(`✅ [DEBUG-NOTIFY] Update callback #${index} completed successfully`);
      } catch (error) {
        console.error(`❌ [DEBUG-NOTIFY] Update callback #${index} failed:`, error);
      }
    });
  }
  
  // Check connection status
  isConnected(): boolean {
    return this.connected && this.connection !== null;
  }
  
  // Subscribe to table updates
  onCheckboxChunkInsert(callback: (chunk: CheckboxChunk) => void): void {
    console.log(`🔔 [DEBUG-REG] Registering insert callback. Total callbacks: ${this.insertCallbacks.length + 1}`);
    this.insertCallbacks.push(callback);
  }
  
  onCheckboxChunkUpdate(callback: (chunk: CheckboxChunk) => void): void {
    console.log(`🔔 [DEBUG-REG] Registering update callback. Total callbacks: ${this.updateCallbacks.length + 1}`);
    this.updateCallbacks.push(callback);
  }
  
  // Call reducers using v2.0 API
  async updateCheckbox(chunk_id: number, bit_offset: number, checked: boolean): Promise<void> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      console.log(`🔄 [DEBUG-1] About to call reducer updateCheckbox(${chunk_id}, ${bit_offset}, ${checked})`);
      console.log(`🔄 [DEBUG-1] Connection status: connected=${this.connected}, connection=${this.connection ? 'exists' : 'null'}`);
      
      // Call the update_checkbox reducer using v2.0 API with correct parameter names (camelCase)
      const result = await this.connection.reducers.updateCheckbox({
        chunkId: chunk_id,
        bitOffset: bit_offset, 
        checked: checked
      });
      
      console.log(`✅ [DEBUG-2] Reducer call completed successfully`);
      console.log(`✅ [DEBUG-2] Result:`, result);
      console.log(`✅ [DEBUG-2] Updated checkbox ${chunk_id}:${bit_offset} = ${checked}`);
    } catch (error) {
      console.error('❌ [DEBUG-ERROR] Failed to update checkbox:', error);
      console.error('❌ [DEBUG-ERROR] Error type:', typeof error);
      console.error('❌ [DEBUG-ERROR] Error details:', error);
      throw error;
    }
  }
  
  async addChunk(chunk_id: number): Promise<void> {
    if (!this.connected || !this.connection) {
      throw new Error('Not connected to SpacetimeDB');
    }
    
    try {
      // Call the add_chunk reducer using v2.0 API
      await this.connection.reducers.addChunk({ chunkId: chunk_id });
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
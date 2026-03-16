# 100×100 Collaborative Checkbox Grid

A real-time collaborative checkbox application with persistence using SpacetimeDB v2.0 and deployed on Netlify.

## 🚀 Live Demo

**Production Site**: https://checkbox-grid-100x100.netlify.app

## ✨ Features

- **100×100 Checkbox Grid** - 10,000 interactive checkboxes with smooth scrolling navigation
- **Real-time Collaboration** - Multiple users can edit simultaneously with instant synchronization
- **Persistence** - All checkbox states are automatically saved and restored across sessions
- **Arrow Key Navigation** - Navigate the grid efficiently with keyboard controls
- **SpacetimeDB Integration** - Powered by SpacetimeDB v2.0 for real-time database synchronization

## 🏗️ Architecture

### Frontend (TypeScript + Vite)
- **Location**: `typescript-frontend/`
- **Technology**: TypeScript, HTML5 Canvas, Vite build system
- **Features**: Virtual viewport rendering, real-time SpacetimeDB client, responsive UI

### Backend (Rust + SpacetimeDB)
- **Location**: `backend/`
- **Technology**: SpacetimeDB v2.0 module in Rust
- **Features**: Efficient chunk-based storage, real-time subscriptions, automatic persistence

### Key Components
- **CheckboxChunk Table**: Stores 1M checkbox states per chunk using bit-packed arrays
- **Real-time Sync**: WebSocket connections with SpacetimeDB cloud infrastructure
- **Canvas Renderer**: Hardware-accelerated rendering for smooth 100×100 grid interaction

## 🚀 Quick Start

### Prerequisites
- Node.js 18+
- Rust 1.94+ (for backend development)
- SpacetimeDB CLI (for backend development)

### Development Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/aharvey101/collaborative-checkboxes.git
   cd collaborative-checkboxes
   ```

2. **Frontend Development** 
   ```bash
   cd typescript-frontend
   npm install
   npm run dev
   ```
   Opens development server at http://localhost:5173

3. **Backend Development** (optional, uses production DB by default)
   ```bash
   cd backend
   spacetime build
   spacetime start
   ```

### Production Deployment

The application automatically deploys to Netlify on every push to main branch:
- **Build Command**: `npm ci && npm run build`
- **Build Directory**: `typescript-frontend/dist/`
- **Live URL**: https://checkbox-grid-100x100.netlify.app

## 🎯 How It Works

1. **User opens application**: Connects to SpacetimeDB production database
2. **Real-time subscriptions**: Establishes WebSocket connection for live updates  
3. **Click checkbox**: Updates local state + sends change to SpacetimeDB
4. **Database sync**: SpacetimeDB persists change and broadcasts to all connected users
5. **Cross-user updates**: Other users see changes instantly in their browser

## 🧪 Verification

The application includes comprehensive testing for core functionality:

- **Connection Test**: Verifies SpacetimeDB v2.0 API connectivity
- **Persistence Test**: Confirms checkbox states survive page reloads
- **Collaboration Test**: Validates real-time synchronization between browser tabs
- **Integration Test**: End-to-end functionality verification

## 📦 Technical Details

### SpacetimeDB Schema
```rust
#[table(accessor = checkbox_chunk, public)]
pub struct CheckboxChunk {
    #[primary_key]
    chunk_id: u32,
    state: Vec<u8>,    // Bit-packed checkbox states (125KB per 1M checkboxes)
    version: u64,      // For optimistic concurrency control
}
```

### Build Configuration
- **Framework**: Vite with TypeScript
- **Deployment**: Netlify with automatic GitHub integration
- **Database**: SpacetimeDB cloud production instance
- **CSP Policy**: Configured for WebAssembly and WebSocket support

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

MIT License - see LICENSE file for details

## 🔗 Links

- [SpacetimeDB Documentation](https://spacetimedb.com/docs)
- [Live Application](https://checkbox-grid-100x100.netlify.app)
- [GitHub Repository](https://github.com/aharvey101/collaborative-checkboxes)

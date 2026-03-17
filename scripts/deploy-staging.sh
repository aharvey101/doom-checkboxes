#!/bin/bash

set -euo pipefail

echo "🚀 Deploying to staging environment..."

# Check prerequisites
if ! command -v spacetime &> /dev/null; then
    echo "❌ SpacetimeDB CLI not found"
    exit 1
fi

if [[ -z "${SPACETIMEDB_TOKEN:-}" ]]; then
    echo "❌ SPACETIMEDB_TOKEN environment variable not set"
    exit 1
fi

# Switch to staging configuration
echo "🔧 Switching to staging configuration..."
node "$(dirname "$0")/test-db-manager.js" switch staging

# Build backend module
echo "🏗️ Building backend module..."
cd "$(dirname "$0")/../backend"

if ! cargo build --release --target wasm32-unknown-unknown; then
    echo "❌ Failed to build backend module"
    exit 1
fi

# Deploy to staging
echo "📦 Deploying to staging database..."
if ! spacetime publish --name collaborative-checkboxes-staging; then
    echo "❌ Failed to deploy to staging"
    exit 1
fi

echo "✅ Successfully deployed to staging"

# Restore original configuration
echo "🔄 Restoring original configuration..."
node "$(dirname "$0")/test-db-manager.js" restore

echo "🎉 Staging deployment completed successfully!"
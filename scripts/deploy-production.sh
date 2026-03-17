#!/bin/bash

set -euo pipefail

echo "🚀 Deploying to production environment..."

# Check prerequisites
if ! command -v spacetime &> /dev/null; then
    echo "❌ SpacetimeDB CLI not found"
    exit 1
fi

if [[ -z "${SPACETIMEDB_TOKEN:-}" ]]; then
    echo "❌ SPACETIMEDB_TOKEN environment variable not set"
    exit 1
fi

# Verify we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [[ "$CURRENT_BRANCH" != "main" ]]; then
    echo "❌ Production deployment only allowed from main branch (current: $CURRENT_BRANCH)"
    exit 1
fi

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo "❌ Uncommitted changes detected. Commit all changes before deployment."
    exit 1
fi

# Switch to production configuration (should be no-op since main uses production)
echo "🔧 Verifying production configuration..."
node "$(dirname "$0")/test-db-manager.js" switch production

# Build backend module
echo "🏗️ Building backend module..."
cd "$(dirname "$0")/../backend"

if ! cargo build --release --target wasm32-unknown-unknown; then
    echo "❌ Failed to build backend module"
    exit 1
fi

# Deploy to production
echo "📦 Deploying to production database..."
if ! spacetime publish --name collaborative-checkboxes-prod; then
    echo "❌ Failed to deploy to production"
    exit 1
fi

echo "✅ Successfully deployed to production"

# Create deployment tag
echo "🏷️ Creating deployment tag..."
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
git tag "prod-deploy-$TIMESTAMP"
git push origin "prod-deploy-$TIMESTAMP"

echo "🎉 Production deployment completed successfully!"
echo "📌 Deployment tag: prod-deploy-$TIMESTAMP"
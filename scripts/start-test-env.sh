#!/bin/bash
# Start SpacetimeDB and publish the backend module, then start trunk.
# Used by playwright.config.ts webServer.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Check if SpacetimeDB is already running on port 3000
if curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:3000/database/ping 2>/dev/null | grep -q "200\|404"; then
    echo "[test-env] SpacetimeDB already running on :3000"
else
    echo "[test-env] Starting SpacetimeDB..."
    spacetime start --listen-addr 127.0.0.1:3000 &

    # Wait for SpacetimeDB to be ready
    for i in $(seq 1 30); do
        if curl -s -o /dev/null http://127.0.0.1:3000 2>/dev/null; then
            echo "[test-env] SpacetimeDB ready"
            break
        fi
        sleep 1
    done
fi

# Publish backend module
echo "[test-env] Publishing backend module..."
cd "$PROJECT_ROOT/backend"
echo "y" | spacetime publish --server local --delete-data checkboxes 2>&1 || true
echo "[test-env] Backend published"

# Start trunk frontend (this blocks and is what playwright monitors)
cd "$PROJECT_ROOT/frontend-rust"
exec trunk serve

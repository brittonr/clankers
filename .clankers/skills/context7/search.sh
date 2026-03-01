#!/usr/bin/env bash
# Context7 library search — no dependencies beyond curl
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: search.sh <library-name>" >&2
    exit 1
fi

QUERY="$1"

RESPONSE=$(curl -s -X POST "https://mcp.context7.com/mcp" \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d "{
        \"jsonrpc\": \"2.0\",
        \"id\": 1,
        \"method\": \"tools/call\",
        \"params\": {
            \"name\": \"resolve-library-id\",
            \"arguments\": {
                \"query\": \"$QUERY\",
                \"libraryName\": \"$QUERY\"
            }
        }
    }")

# Check for errors
if echo "$RESPONSE" | grep -q '"error"'; then
    echo "Error: $(echo "$RESPONSE" | grep -o '"message":"[^"]*"')" >&2
    exit 1
fi

# Extract and print the content text
echo "$RESPONSE" | grep -o '"text":"[^"]*"' | head -1 | sed 's/"text":"//;s/"$//' | sed 's/\\n/\n/g'

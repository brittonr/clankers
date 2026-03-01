#!/usr/bin/env bash
# Context7 documentation query — no dependencies beyond curl
set -euo pipefail

if [ $# -lt 2 ]; then
    echo "Usage: docs.sh <library-id> <query>" >&2
    echo "Example: docs.sh /facebook/react 'useEffect cleanup'" >&2
    exit 1
fi

LIBRARY_ID="$1"
QUERY="$2"

RESPONSE=$(curl -s -X POST "https://mcp.context7.com/mcp" \
    -H "Content-Type: application/json" \
    -H "Accept: application/json, text/event-stream" \
    -d "{
        \"jsonrpc\": \"2.0\",
        \"id\": 1,
        \"method\": \"tools/call\",
        \"params\": {
            \"name\": \"query-docs\",
            \"arguments\": {
                \"libraryId\": \"$LIBRARY_ID\",
                \"query\": \"$QUERY\"
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

#!/bin/bash
# Test z.ai Coding API (GLM 4.7) connectivity

set -e

# Load .env if present
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

if [ -z "$R_DATA_AGENT_API_KEY" ]; then
    echo "Error: R_DATA_AGENT_API_KEY not set. Add it to .env or export it."
    exit 1
fi

echo "Testing z.ai Coding API (GLM 4.7)..."
echo ""

response=$(curl -s -w "\n%{http_code}" -X POST "https://api.z.ai/api/coding/paas/v4/chat/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $R_DATA_AGENT_API_KEY" \
  -d '{
    "model": "glm-4.7",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant. Reply with exactly: OK"},
      {"role": "user", "content": "Say OK"}
    ],
    "stream": false
  }')

# Split response body and status code
http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | sed '$d')

if [ "$http_code" = "200" ]; then
    echo "✓ API test passed (HTTP 200)"
    content=$(echo "$body" | grep -o '"content":"[^"]*"' | head -1 | sed 's/"content":"//;s/"$//')
    if [ -n "$content" ]; then
        echo "✓ Response: $content"
    fi
    echo ""
    echo "API is working correctly."
else
    echo "✗ API test failed (HTTP $http_code)"
    echo "Response: $body"
    exit 1
fi

#!/bin/bash
# Test Z.AI Coding Plan API key
# Uses the coding plan endpoint: https://api.z.ai/api/coding/paas/v4

set -e
source .env 2>/dev/null || true

API_KEY="${ZAI_API_KEY:-${ZHIPU_API_KEY}}"
if [ -z "$API_KEY" ]; then
  echo "Error: Set ZAI_API_KEY or ZHIPU_API_KEY in .env"
  exit 1
fi

echo "Testing Z.AI Coding Plan API..."
echo "Endpoint: https://api.z.ai/api/coding/paas/v4/chat/completions"
echo "Model: glm-4.7-flash"
echo ""

RESP=$(curl -s -w "\n%{http_code}" -X POST "https://api.z.ai/api/coding/paas/v4/chat/completions" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $API_KEY" \
  -d '{
    "model": "glm-4.7-flash",
    "messages": [{"role": "user", "content": "Say hello in one word."}],
    "stream": false,
    "max_tokens": 50
  }')

HTTP_CODE=$(echo "$RESP" | tail -n1)
BODY=$(echo "$RESP" | sed '$d')

if [ "$HTTP_CODE" = "200" ]; then
  echo "✓ API key valid!"
  CONTENT=$(echo "$BODY" | jq -r '.choices[0].message.content // .error.message // .message // "?"' 2>/dev/null || echo "$BODY")
  echo "Response: $CONTENT"
else
  echo "✗ API error (HTTP $HTTP_CODE)"
  echo "$BODY" | jq . 2>/dev/null || echo "$BODY"
  exit 1
fi

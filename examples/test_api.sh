#!/bin/bash

# Test script for the graph-service API

BASE_URL="http://localhost:3000"

echo "Testing graph-service API..."
echo

# Health check
echo "1. Health check:"
curl -s "$BASE_URL/health"
echo -e "\n"

# Execute graph without session ID (creates new session)
echo "2. Execute graph (new session):"
RESPONSE=$(curl -s -X POST "$BASE_URL/execute" \
  -H "Content-Type: application/json" \
  -d '{
    "content": "How can I improve my search for machine learning papers?"
  }')

echo "$RESPONSE" | jq .
SESSION_ID=$(echo "$RESPONSE" | jq -r .session_id)
echo

# Execute graph with existing session ID
echo "3. Execute graph (existing session):"
curl -s -X POST "$BASE_URL/execute" \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID\",
    \"content\": \"Tell me more about neural networks\"
  }" | jq .
echo

# Get session details
echo "4. Get session details:"
curl -s "$BASE_URL/session/$SESSION_ID" | jq .
echo

echo "Test completed!" 
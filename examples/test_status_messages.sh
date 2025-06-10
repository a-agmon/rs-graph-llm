#!/bin/bash

# Test script to demonstrate the new status message functionality

echo "=== Testing Status Message Feature ==="
echo ""

# Start a new session
echo "1. Starting new session with Hi..."
RESPONSE=$(curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Hi"
  }')

echo "Response: $RESPONSE"

# Extract session ID
SESSION_ID=$(echo $RESPONSE | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
echo "Session ID: $SESSION_ID"
echo ""

# Check initial status
echo "2. Checking initial session status..."
curl -s -X GET "http://localhost:3000/session/$SESSION_ID" | jq .
echo ""

# Provide username
echo "3. Providing username..."
curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID\",
    \"content\": \"My username is john_doe\"
  }" | jq .
echo ""

# Check status after username
echo "4. Checking status after username..."
curl -s -X GET "http://localhost:3000/session/$SESSION_ID" | jq .
echo ""

# Provide bank number to complete the flow
echo "5. Providing bank number..."
curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID\",
    \"content\": \"My bank number is 1234567891\"
  }" | jq .
echo ""

# Check final status - should show completed user details and move to account fetch
echo "6. Checking for session status after bank number..."
curl -s -X GET "http://localhost:3000/session/$SESSION_ID" | jq .
echo ""

echo "7. Asking questions about the account..."
curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID\",
    \"content\": \"what is my balance?\"
  }" | jq .
echo ""

echo "8. final session status..."
curl -s -X GET "http://localhost:3000/session/$SESSION_ID" | jq .
echo ""
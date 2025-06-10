#!/bin/bash

# Test script for Insurance Claims Workflow

echo "=== Testing Insurance Claims Workflow ==="
echo ""

# Start a new session with initial claim
echo "1. Starting new insurance claim..."
RESPONSE=$(curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{
    "content": "I need to file an insurance claim for damage to my car"
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

# Specify insurance type
echo "3. Clarifying insurance type..."
curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID\",
    \"content\": \"This is for my car insurance\"
  }" | jq .
echo ""

# Check status after insurance type classification
echo "4. Checking status after insurance type..."
curl -s -X GET "http://localhost:3000/session/$SESSION_ID" | jq .
echo ""

# Provide car insurance details
echo "5. Providing car damage details..."
curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID\",
    \"content\": \"I was in a collision and my front bumper was damaged. The repair estimate is about 800 dollars.\"
  }" | jq .
echo ""

# Check status after details collection
echo "6. Checking status after car details..."
curl -s -X GET "http://localhost:3000/session/$SESSION_ID" | jq .
echo ""

echo "=== Testing High-Value Claim (Manual Approval) ==="
echo ""

# Start another session for high-value claim
echo "7. Starting high-value apartment claim..."
RESPONSE2=$(curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{
    "content": "I need to file a claim for water damage in my apartment"
  }')

SESSION_ID2=$(echo $RESPONSE2 | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
echo "High-value claim Session ID: $SESSION_ID2"
echo ""

# Specify apartment insurance
echo "8. Specifying apartment insurance..."
curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID2\",
    \"content\": \"This is for my apartment insurance\"
  }" | jq .
echo ""

# Provide high-value apartment details
echo "9. Providing apartment damage details (high value)..."
curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID2\",
    \"content\": \"Water pipe burst and flooded my apartment. Damaged furniture, electronics, and hardwood floors. Estimated cost is 2500 dollars.\"
  }" | jq .
echo ""

# Approve the claim
echo "10. Approving the high-value claim..."
curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID2\",
    \"content\": \"approve\"
  }" | jq .
echo ""

# Check final status
echo "11. Checking final status for high-value claim..."
curl -s -X GET "http://localhost:3000/session/$SESSION_ID2" | jq .
echo ""

echo "=== Test Complete ==="
#!/bin/bash

# Demo script showing session ID validation

BASE_URL="http://localhost:3000"

echo "=== Session ID Validation Demo ==="
echo

# 1. Test with valid new session (no session_id provided)
echo "1. Creating new session (no session_id):"
RESPONSE=$(curl -s -X POST "$BASE_URL/execute" \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Hi, I want to check my bank account."
  }')

echo "$RESPONSE" | jq .
SESSION_ID=$(echo "$RESPONSE" | jq -r .session_id)
echo "✅ Valid session created: $SESSION_ID"
echo

# 2. Test with valid existing session ID (correct usage)
echo "2. Using valid existing session ID:"
curl -s -X POST "$BASE_URL/execute" \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$SESSION_ID\",
    \"content\": \"My username is john_doe and my account number is 1234567891\"
  }" | jq .
echo "✅ Valid session ID accepted"
echo

# 3. Test with invalid session ID format (will fail now)
echo "3. Using invalid session ID format (should fail):"
curl -s -X POST "$BASE_URL/execute" \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "$R_SESSION",
    "content": "This should fail"
  }'
echo
echo "❌ Invalid session ID format rejected"
echo

# 4. Test with valid UUID format but non-existent session (will fail now)
echo "4. Using valid UUID format but non-existent session (should fail):"
FAKE_UUID="12345678-1234-1234-1234-123456789012"
curl -s -X POST "$BASE_URL/execute" \
  -H "Content-Type: application/json" \
  -d "{
    \"session_id\": \"$FAKE_UUID\",
    \"content\": \"This should also fail\"
  }"
echo
echo "❌ Non-existent session ID rejected"
echo

# 5. Show the difference between single and double quotes
echo "5. Shell variable expansion demo:"
R_SESSION="example-variable"
echo "Using single quotes (no expansion): '\$R_SESSION' = \$R_SESSION"
echo "Using double quotes (with expansion): \"\$R_SESSION\" = $R_SESSION"
echo

echo "=== Demo completed ==="
echo
echo "Key takeaways:"
echo "- Use double quotes in JSON when you need shell variable expansion"
echo "- Server now validates session ID format (must be valid UUID)"
echo "- Server now returns 404 for non-existent but valid session IDs"
echo "- Server now returns 400 for invalid session ID formats" 
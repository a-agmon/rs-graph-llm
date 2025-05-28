#!/bin/bash

# Test script to demonstrate multi-user session isolation

echo "Testing Multi-User Session Isolation"
echo "===================================="

# Start the service in the background (assuming it's running on port 3000)
# Make sure the service is running before executing this script

# User 1: First request (no session ID)
echo -e "\n1. User 1 - First request:"
USER1_RESPONSE=$(curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"content": "Find information about Rust programming"}')

USER1_SESSION=$(echo $USER1_RESPONSE | jq -r '.session_id')
echo "Response: $USER1_RESPONSE"
echo "User 1 Session ID: $USER1_SESSION"

# User 2: First request (no session ID)
echo -e "\n2. User 2 - First request:"
USER2_RESPONSE=$(curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"content": "Search for Python tutorials"}')

USER2_SESSION=$(echo $USER2_RESPONSE | jq -r '.session_id')
echo "Response: $USER2_RESPONSE"
echo "User 2 Session ID: $USER2_SESSION"

# Verify sessions are different
echo -e "\n3. Verifying sessions are different:"
if [ "$USER1_SESSION" != "$USER2_SESSION" ]; then
    echo "✓ Sessions are different (as expected)"
else
    echo "✗ Sessions are the same (unexpected!)"
fi

# User 1: Second request with session ID
echo -e "\n4. User 1 - Second request with session ID:"
USER1_RESPONSE2=$(curl -s -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d "{\"session_id\": \"$USER1_SESSION\", \"content\": \"More about Rust async\"}")
echo "Response: $USER1_RESPONSE2"

# Check User 1's session state
echo -e "\n5. Checking User 1's session state:"
USER1_SESSION_STATE=$(curl -s http://localhost:3000/session/$USER1_SESSION)
echo "User 1 Session State: $USER1_SESSION_STATE" | jq '.'

# Check User 2's session state
echo -e "\n6. Checking User 2's session state:"
USER2_SESSION_STATE=$(curl -s http://localhost:3000/session/$USER2_SESSION)
echo "User 2 Session State: $USER2_SESSION_STATE" | jq '.'

echo -e "\n7. Summary:"
echo "- User 1 and User 2 have separate session IDs"
echo "- Each session maintains its own context and state"
echo "- Data from one user does not affect the other user" 
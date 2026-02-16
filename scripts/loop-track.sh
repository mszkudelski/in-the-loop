#!/bin/bash
# Usage: loop-track <command...>
# Example: loop-track gh copilot suggest "how to parse JSON in rust"

LOOP_TRACKER_PORT=19532
COMMAND="$*"
TITLE="CLI: $COMMAND"

# Register session
RESPONSE=$(curl -s -X POST "http://localhost:$LOOP_TRACKER_PORT/api/sessions" \
  -H "Content-Type: application/json" \
  -d "{\"command\": \"$COMMAND\", \"title\": \"$TITLE\"}")

SESSION_ID=$(echo "$RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

if [ -z "$SESSION_ID" ] || [ "$SESSION_ID" = "error" ]; then
  echo "Warning: Failed to register with In The Loop tracker"
  echo "Make sure the app is running"
fi

# Run the actual command
eval "$COMMAND"
EXIT_CODE=$?

# Update status
if [ -n "$SESSION_ID" ] && [ "$SESSION_ID" != "error" ]; then
  if [ $EXIT_CODE -eq 0 ]; then
    STATUS="completed"
  else
    STATUS="failed"
  fi

  curl -s -X PATCH "http://localhost:$LOOP_TRACKER_PORT/api/sessions/$SESSION_ID" \
    -H "Content-Type: application/json" \
    -d "{\"status\": \"$STATUS\"}" > /dev/null
fi

exit $EXIT_CODE

#!/bin/bash
# Usage: loop-track <command...>
# Example: loop-track copilot "how to parse JSON in rust"

LOOP_TRACKER_PORT=19532
COMMAND="$*"
TITLE="CLI: $COMMAND"

update_session_status() {
  local session_id="$1"
  local status="$2"

  if [ -n "$session_id" ] && [ "$session_id" != "error" ]; then
    curl -s -X PATCH "http://localhost:$LOOP_TRACKER_PORT/api/sessions/$session_id" \
      -H "Content-Type: application/json" \
      -d "{\"status\": \"$status\"}" > /dev/null
  fi
}

is_interactive_copilot_command() {
  case "$1" in
    copilot*|"gh copilot"*) return 0 ;;
    *) return 1 ;;
  esac
}

log_indicates_copilot_waiting() {
  local log_file="$1"

  local has_idle_prompt=0
  local has_activity=0

  if grep -aEiq 'Type @ to mention files|/ for commands, or \? for shortcuts|waiting.*input|what.*next|next.*task|enter.*(prompt|message)' "$log_file"; then
    has_idle_prompt=1
  fi

  if grep -aEiq '◐|◓|◑|◒|The user wants|I.ll explore|● Read |● List |● Summary' "$log_file"; then
    has_activity=1
  fi

  [ "$has_idle_prompt" -eq 1 ] && [ "$has_activity" -eq 1 ]
}

# Register session
RESPONSE=$(curl -s -X POST "http://localhost:$LOOP_TRACKER_PORT/api/sessions" \
  -H "Content-Type: application/json" \
  -d "{\"command\": \"$COMMAND\", \"title\": \"$TITLE\"}")

SESSION_ID=$(echo "$RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

if [ -z "$SESSION_ID" ] || [ "$SESSION_ID" = "error" ]; then
  echo "Warning: Failed to register with In The Loop tracker"
  echo "Make sure the app is running"
fi

# For interactive Copilot sessions, preserve TTY while capturing output
# via `script`, then auto-mark complete when Copilot indicates it is
# waiting for next user input.
AUTO_COMPLETED=0

if is_interactive_copilot_command "$COMMAND"; then
  OUTPUT_LOG=$(mktemp)
  AUTO_COMPLETE_FLAG=$(mktemp)
  touch "$OUTPUT_LOG"
  echo "0" > "$AUTO_COMPLETE_FLAG"

  while true; do
    if [ "$(cat "$AUTO_COMPLETE_FLAG")" = "0" ] && log_indicates_copilot_waiting "$OUTPUT_LOG"; then
      update_session_status "$SESSION_ID" "completed"
      echo "1" > "$AUTO_COMPLETE_FLAG"
      break
    fi
    sleep 1
  done &
  MONITOR_PID=$!

  if command -v script >/dev/null 2>&1; then
    USER_SHELL="${SHELL:-/bin/zsh}"
    script -q "$OUTPUT_LOG" "$USER_SHELL" -ic "$COMMAND"
    EXIT_CODE=$?
  else
    eval "$COMMAND"
    EXIT_CODE=$?
  fi

  kill "$MONITOR_PID" 2>/dev/null || true

  if [ "$(cat "$AUTO_COMPLETE_FLAG")" = "1" ]; then
    AUTO_COMPLETED=1
  fi

  rm -f "$OUTPUT_LOG" "$AUTO_COMPLETE_FLAG"
else
  eval "$COMMAND"
  EXIT_CODE=$?
fi

# Update status
if [ $AUTO_COMPLETED -eq 1 ]; then
  STATUS="completed"
elif [ $EXIT_CODE -eq 0 ]; then
  STATUS="completed"
else
  STATUS="failed"
fi

update_session_status "$SESSION_ID" "$STATUS"

exit $EXIT_CODE

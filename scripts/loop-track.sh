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

# Register session (include cwd for copilot session matching)
CWD="$(pwd)"
RESPONSE=$(curl -s -X POST "http://localhost:$LOOP_TRACKER_PORT/api/sessions" \
  -H "Content-Type: application/json" \
  -d "{\"command\": \"$COMMAND\", \"title\": \"$TITLE\", \"cwd\": \"$CWD\"}")

SESSION_ID=$(echo "$RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)

if [ -z "$SESSION_ID" ] || [ "$SESSION_ID" = "error" ]; then
  echo "Warning: Failed to register with In The Loop tracker"
  echo "Make sure the app is running"
fi

# For interactive Copilot sessions, preserve TTY while capturing output
# via `script`. Status detection is handled by the app's polling system
# through events.jsonl, so no log monitoring is needed here.

if is_interactive_copilot_command "$COMMAND"; then
  # Update to in_progress immediately
  update_session_status "$SESSION_ID" "in_progress"

  if command -v script >/dev/null 2>&1; then
    OUTPUT_LOG=$(mktemp)
    USER_SHELL="${SHELL:-/bin/zsh}"
    script -q "$OUTPUT_LOG" "$USER_SHELL" -ic "$COMMAND"
    EXIT_CODE=$?
    rm -f "$OUTPUT_LOG"
  else
    eval "$COMMAND"
    EXIT_CODE=$?
  fi
else
  eval "$COMMAND"
  EXIT_CODE=$?
fi

# Update status
# For interactive Copilot sessions, let the app's polling system manage status
# via events.jsonl detection. Only set final status for non-copilot commands.
if is_interactive_copilot_command "$COMMAND"; then
  # Don't override — polling detects in_progress/input_needed/completed from events.jsonl
  :
elif [ $EXIT_CODE -eq 0 ]; then
  update_session_status "$SESSION_ID" "completed"
else
  update_session_status "$SESSION_ID" "failed"
fi

exit $EXIT_CODE

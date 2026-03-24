#!/bin/bash
set -euo pipefail

# claude-mode: Read/write session mode and approach state
# State file: /tmp/claude-session-{session_id}
# Format: KEY=VALUE lines (MODE, APPROACH, PROJECT)

SESSION_ID=""
ACTION=""
KEY=""
VALUE=""

usage() {
  echo "Usage:" >&2
  echo "  claude-mode --session <id> init <project_dir>" >&2
  echo "  claude-mode --session <id> get <mode|approach|project|all>" >&2
  echo "  claude-mode --session <id> set <mode|approach> <value>" >&2
  exit 1
}

# Parse args
while [[ $# -gt 0 ]]; do
  case "$1" in
    --session)
      [[ $# -lt 2 ]] && usage
      SESSION_ID="$2"
      shift 2
      ;;
    init|get|set)
      ACTION="$1"
      KEY="${2:-}"
      VALUE="${3:-}"
      break
      ;;
    *)
      usage
      ;;
  esac
done

[[ -z "$SESSION_ID" || -z "$ACTION" ]] && usage

FILE="/tmp/claude-session-${SESSION_ID}"

case "$ACTION" in
  init)
    [[ -z "$KEY" ]] && usage
    cat > "$FILE" <<EOF
MODE=Proposal
APPROACH=Team
PROJECT=${KEY}
EOF
    ;;

  get)
    [[ -z "$KEY" ]] && usage
    [[ ! -f "$FILE" ]] && exit 0
    KEY_UPPER=$(echo "$KEY" | tr '[:lower:]' '[:upper:]')
    if [[ "$KEY" == "all" ]]; then
      cat "$FILE"
    else
      grep "^${KEY_UPPER}=" "$FILE" 2>/dev/null | cut -d= -f2- || true
    fi
    ;;

  set)
    [[ -z "$KEY" || -z "$VALUE" ]] && usage
    KEY_UPPER=$(echo "$KEY" | tr '[:lower:]' '[:upper:]')
    if [[ -f "$FILE" ]] && grep -q "^${KEY_UPPER}=" "$FILE"; then
      sed -i '' "s|^${KEY_UPPER}=.*|${KEY_UPPER}=${VALUE}|" "$FILE"
    else
      echo "${KEY_UPPER}=${VALUE}" >> "$FILE"
    fi
    ;;

  *)
    usage
    ;;
esac

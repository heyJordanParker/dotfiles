#!/bin/bash
# Block modifications to session state files unless classifier is active
# Prevents agents from tampering with enforcement hook state
# Gracefully allows on any error (missing files, parse error)

read -r input

# Allow if inside the classifier
[ "${CLAUDE_CLASSIFY_INTENT:-}" = "true" ] && exit 0

file_path=$(echo "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null) || true
command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null) || true

# Check for protected patterns
[[ "$file_path" == *"claude-session-state"* ]] || \
echo "$command" | grep -qE '(echo|printf|cat|rm|mv|cp|jq|tee|chmod|chown|ls|stat|head|tail|sed|awk)\b.*claude-session-state|[>].*claude-session-state' 2>/dev/null || exit 0

cat >&2 <<'EOF'
BLOCKED: Session state files are managed by the intent classifier.

To change modes, tell the user — e.g. "enter solo mode" or "switch to team".
Do not modify session state files directly.
EOF
exit 2

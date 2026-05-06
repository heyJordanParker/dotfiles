#!/bin/bash
# session-state.sh — unified Claude session state helper.
#
# All hooks read/write per-session state through this script.
# Storage root: $CLAUDE_DATA_ROOT or ~/.claude
# Sessions live at: <root>/sessions/<session_id>/
# Subagents nest at: <root>/sessions/<parent_id>/subagents/<agent_id>/
# Schema version: 1
#
# Subcommands:
#   start <session_id> [--transcript-path <path>]   open/heal session, record session_start
#   end <session_id>                                rm -rf, cascades subagents
#   get <session_id> <field>                        read field from state.json (soft on missing)
#   get --path [target]                             resolve a path; target ∈ {<session_id>, data-root, sessions, shaping}; no target = current $CLAUDE_SESSION_ID
#   set <session_id> <field> <value>                atomic single-field update
#   merge <session_id> <json_fragment>              atomic multi-field update
#   prompt <session_id>                             read prompt from stdin; if human, rotate turn timestamps and bump human_turns
#   stopped <session_id>                            record last_stop (last-write-wins across multi-Stop)
#   tool-used <session_id>                          atomic ++ on tools_used
#   read <session_id> <file_path>                   append entry to reads.jsonl ({path, ts})
#   skill <session_id> <skill_name>                 append entry to skills.jsonl ({skill, ts})
#   compacted <session_id>                          truncate reads.jsonl and skills.jsonl (post-compaction reset)
#   find-by-pane <pane_id>                          default zellij (.pane); --tmux for .tmux-pane
#   list                                            main sessions; --subagents <id> for nested
#   stats <session_id>                              JSON snapshot of session timings + counters
#   is-long-running <session_id> [thresholds]       gate; defaults --turns 5 --seconds 600 --tools 30

set -uo pipefail

# Hard dependency — fail loud rather than silently produce empty state.
if ! command -v jq >/dev/null 2>&1; then
    echo "Error: jq is required by session-state.sh but not found in PATH" >&2
    exit 127
fi

# ============================================================================
# Private helpers
# ============================================================================

_data_root() {
    echo "${CLAUDE_DATA_ROOT:-$HOME/.claude}"
}

_sessions_root() {
    echo "$(_data_root)/sessions"
}

# Where Claude Code writes its transcripts. Subagent transcripts live at
# $CLAUDE_PROJECTS_ROOT/{enc-cwd}/{parent_session_id}/subagents/agent-{id}.jsonl
_projects_root() {
    echo "${CLAUDE_PROJECTS_ROOT:-$HOME/.claude/projects}"
}

# Single chokepoint for "current epoch seconds." Tests can mock by overriding
# the `date` command in PATH.
_now() {
    date +%s
}

_atomic_write() {
    local file="$1"
    local content="$2"
    local dir
    dir=$(dirname "$file")
    mkdir -p "$dir" 2>/dev/null || return 1
    local tmp
    tmp=$(mktemp "${dir}/.session-state.XXXXXX" 2>/dev/null) || return 1
    if printf '%s\n' "$content" > "$tmp" 2>/dev/null; then
        mv "$tmp" "$file" 2>/dev/null || { rm -f "$tmp" 2>/dev/null; return 1; }
        return 0
    fi
    rm -f "$tmp" 2>/dev/null
    return 1
}

# Atomically truncate a file to zero bytes. mktemp + mv replaces the target
# in one rename; readers either see the old content or an empty file, never
# a half-written intermediate. Missing target is fine — mv creates it empty.
_truncate() {
    local file="$1"
    local dir
    dir=$(dirname "$file")
    mkdir -p "$dir" 2>/dev/null || return 1
    local tmp
    tmp=$(mktemp "${dir}/.session-state.XXXXXX" 2>/dev/null) || return 1
    mv "$tmp" "$file" 2>/dev/null || { rm -f "$tmp" 2>/dev/null; return 1; }
}

# Atomic increment of an integer field. Treats missing/null as 0.
_bump() {
    local state_file="$1"
    local field="$2"
    [ ! -f "$state_file" ] && return 1
    local updated
    updated=$(jq --arg f "$field" '.[$f] = ((.[$f] // 0) + 1)' "$state_file" 2>/dev/null)
    [ -z "$updated" ] && return 1
    _atomic_write "$state_file" "$updated"
}

# Reads prompt content from stdin. Returns 0 if human-typed, 1 if system-injected.
# Rules (verified at >92% confidence against ~14k real user-records across 9 projects):
#   1. XML-tagged with matching close tag (e.g. <task-notification>...</task-notification>)
#   2. Single-line bracket-enclosed (e.g. [foo bar])
#   3. Prefix "This session is being continued"  (compaction continuation)
#   4. Prefix "Base directory for this skill:"   (skill expansion)
_human_prompt() {
    local content
    content=$(cat)
    # Empty content isn't a real human prompt — Claude Code never emits empty
    # UserPromptSubmit; counting empty as a turn would corrupt human_turns.
    [ -z "$content" ] && return 1

    # Rule 1: XML-tagged
    if [[ "$content" == "<"* ]]; then
        local rest="${content#<}"
        local tag="${rest%%[> ]*}"
        if [ -n "$tag" ] && [[ "$content" == *"</${tag}>"* ]]; then
            return 1
        fi
    fi

    # Rule 2: single-line bracket-enclosed
    if [[ "$content" != *$'\n'* ]] && [[ "$content" == "["*"]" ]]; then
        return 1
    fi

    # Rule 3: continuation prefix
    [[ "$content" == "This session is being continued"* ]] && return 1

    # Rule 4: skill expansion prefix
    [[ "$content" == "Base directory for this skill:"* ]] && return 1

    return 0
}

_default_main_state() {
    local session_id="$1"
    jq -n --arg sid "$session_id" '{
        session_id: $sid,
        role: "main",
        parent_session_id: null,
        approach: "solo",
        state: "proposing",
        intent: "instructions",
        commit_requested: false,
        notes: [],
        validation_phase: 0,
        pane: null,
        "tmux-pane": null,
        session_start: null,
        human_turns: 0,
        current_turn_start: null,
        previous_turn_start: null,
        last_stop: null,
        tools_used: 0,
        schema_version: 1
    }'
}

_default_subagent_state() {
    local session_id="$1"
    local parent_id="${2:-}"
    jq -n --arg sid "$session_id" --arg pid "$parent_id" '{
        session_id: $sid,
        role: "subagent",
        parent_session_id: (if $pid == "" then null else $pid end),
        pane: null,
        "tmux-pane": null,
        session_start: null,
        human_turns: 0,
        current_turn_start: null,
        previous_turn_start: null,
        last_stop: null,
        tools_used: 0,
        schema_version: 1
    }'
}

_is_valid_session_id() {
    local id="$1"
    [ -z "$id" ] && return 1
    [[ "$id" =~ ^[A-Za-z0-9_][A-Za-z0-9_-]*$ ]]
}

# Subagent transcripts: .../{parent_session_id}/subagents/agent-{id}.jsonl
# Multi-/subagents/ paths are rejected (Claude Code doesn't nest subagents).
_parse_parent_from_transcript() {
    local path="$1"
    if [[ "$path" == */subagents/agent-*.jsonl ]]; then
        local count
        count=$(awk 'BEGIN{n=0}{while(match($0,"/subagents/")){n++; $0=substr($0,RSTART+RLENGTH)}}END{print n}' <<< "$path")
        [ "$count" -ne 1 ] && return 0
        local parent
        parent=$(basename "$(dirname "$(dirname "$path")")")
        if _is_valid_session_id "$parent"; then
            echo "$parent"
        fi
    fi
}

_session_dir() {
    local session_id="$1"
    local sessions_root
    sessions_root="$(_sessions_root)"
    local main_dir="${sessions_root}/${session_id}"
    if [ -d "$main_dir" ]; then
        echo "$main_dir"
        return 0
    fi
    local match
    match=$(find "$sessions_root" -mindepth 3 -maxdepth 3 -type d -name "$session_id" -path "*/subagents/*" 2>/dev/null | head -1)
    if [ -n "$match" ]; then
        echo "$match"
        return 0
    fi
    return 1
}

# Resolve a subagent's parent_session_id by globbing on-disk transcripts.
_resolve_parent_id() {
    local session_id="$1"
    [[ "$session_id" == agent-* ]] || return 0

    local projects_root
    projects_root="$(_projects_root)"
    [ ! -d "$projects_root" ] && return 0

    shopt -s nullglob
    local matches=("$projects_root"/*/*/subagents/"${session_id}.jsonl")
    shopt -u nullglob

    [ "${#matches[@]}" -ne 1 ] && return 0

    local parent
    parent=$(basename "$(dirname "$(dirname "${matches[0]}")")")
    if _is_valid_session_id "$parent"; then
        echo "$parent"
    fi
}

# Resolve-or-create a session dir + state.json. Single source of truth.
_ensure_session() {
    local session_id="$1"
    local parent_override="${2:-}"

    local resolved_parent="$parent_override"
    if [ -z "$resolved_parent" ]; then
        resolved_parent=$(_resolve_parent_id "$session_id")
    fi

    # Case-collision check for top-level mains (case-insensitive APFS guard)
    if [ -z "$resolved_parent" ]; then
        local sessions_root_path
        sessions_root_path="$(_sessions_root)"
        if [ -d "$sessions_root_path" ]; then
            local session_id_lower
            session_id_lower=$(printf '%s' "$session_id" | tr '[:upper:]' '[:lower:]')
            shopt -s nullglob
            local entry entry_name on_disk=""
            for entry in "$sessions_root_path"/*/; do
                entry_name=$(basename "$entry")
                if [ "$(printf '%s' "$entry_name" | tr '[:upper:]' '[:lower:]')" = "$session_id_lower" ]; then
                    on_disk="$entry_name"
                    break
                fi
            done
            shopt -u nullglob
            if [ -n "$on_disk" ] && [ "$on_disk" != "$session_id" ]; then
                echo "Error: session_id '$session_id' collides case-insensitively with existing '$on_disk'" >&2
                return 1
            fi
        fi
    fi

    # Already exists — heal corrupt/missing/empty state.json in place
    local session_dir
    if session_dir=$(_session_dir "$session_id" 2>/dev/null); then
        local state_file="${session_dir}/state.json"
        local needs_defaults=false
        if [ ! -f "$state_file" ]; then
            needs_defaults=true
        elif [ ! -s "$state_file" ]; then
            needs_defaults=true
        elif ! jq -e 'type == "object"' "$state_file" >/dev/null 2>&1; then
            needs_defaults=true
        fi
        if [ "$needs_defaults" = true ]; then
            local default_state
            if [[ "$session_dir" == */subagents/* ]]; then
                local existing_parent
                existing_parent=$(basename "$(dirname "$(dirname "$session_dir")")")
                default_state=$(_default_subagent_state "$session_id" "$existing_parent")
            else
                default_state=$(_default_main_state "$session_id")
            fi
            _atomic_write "$state_file" "$default_state"
        fi
        echo "$session_dir"
        return 0
    fi

    # New session — agent-* without resolvable parent fails loud
    if [[ "$session_id" == agent-* ]] && [ -z "$resolved_parent" ]; then
        echo "Error: subagent session_id '$session_id' has no resolvable parent (transcript not found under \$CLAUDE_PROJECTS_ROOT)" >&2
        return 1
    fi

    if [ -n "$resolved_parent" ]; then
        session_dir="$(_sessions_root)/${resolved_parent}/subagents/${session_id}"
    else
        session_dir="$(_sessions_root)/${session_id}"
    fi

    mkdir -p "$session_dir"

    local state_file="${session_dir}/state.json"
    local default_state
    if [ -n "$resolved_parent" ]; then
        default_state=$(_default_subagent_state "$session_id" "$resolved_parent")
    else
        default_state=$(_default_main_state "$session_id")
    fi
    _atomic_write "$state_file" "$default_state"

    echo "$session_dir"
    return 0
}

# ============================================================================
# Public subcommands
# ============================================================================

cmd_start() {
    [ $# -lt 1 ] && { echo "Error: start requires session_id" >&2; return 1; }
    local session_id="$1"
    shift

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local transcript_path=""
    while [ $# -gt 0 ]; do
        case "$1" in
            --transcript-path)
                if [ $# -lt 2 ]; then
                    echo "Error: --transcript-path requires a value" >&2
                    return 1
                fi
                transcript_path="$2"
                shift 2
                ;;
            *)
                echo "Error: unknown start flag: $1" >&2
                return 1
                ;;
        esac
    done

    local parent_override=""
    if [ -n "$transcript_path" ]; then
        parent_override=$(_parse_parent_from_transcript "$transcript_path")
    fi

    local session_dir
    session_dir=$(_ensure_session "$session_id" "$parent_override") || return 1

    # One-shot: record session_start the first time start runs for this session
    local state_file="${session_dir}/state.json"
    local current_start
    current_start=$(jq -r '.session_start // empty' "$state_file" 2>/dev/null)
    if [ -z "$current_start" ]; then
        local now updated
        now=$(_now)
        updated=$(jq --argjson now "$now" '.session_start = $now' "$state_file" 2>/dev/null)
        [ -n "$updated" ] && _atomic_write "$state_file" "$updated"
    fi

    if [ -n "$transcript_path" ]; then
        _atomic_write "${session_dir}/transcript" "$transcript_path"
    fi
}

cmd_end() {
    [ $# -lt 1 ] && { echo "Error: end requires session_id" >&2; return 1; }
    local session_id="$1"
    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }
    local session_dir
    if session_dir=$(_session_dir "$session_id" 2>/dev/null); then
        rm -rf "$session_dir"
    fi
    return 0
}

cmd_get() {
    # Path-resolution mode
    if [ $# -gt 0 ] && [ "$1" = "--path" ]; then
        shift
        if [ $# -eq 0 ]; then
            if [ -z "${CLAUDE_SESSION_ID:-}" ]; then
                echo "Error: CLAUDE_SESSION_ID not set" >&2
                return 1
            fi
            _is_valid_session_id "$CLAUDE_SESSION_ID" || {
                echo "Error: invalid CLAUDE_SESSION_ID: $CLAUDE_SESSION_ID" >&2
                return 1
            }
            _session_dir "$CLAUDE_SESSION_ID"
            return $?
        fi
        case "$1" in
            data-root) _data_root ;;
            sessions)  _sessions_root ;;
            shaping)   echo "$(_data_root)/shaping" ;;
            *)
                _is_valid_session_id "$1" || {
                    echo "Error: invalid path target: $1" >&2
                    return 1
                }
                _session_dir "$1"
                ;;
        esac
        return $?
    fi

    # Field-read mode (existing semantics)
    [ $# -lt 2 ] && { echo "Error: get requires session_id + field, or --path [target]" >&2; return 1; }
    local session_id="$1"
    local field="$2"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local session_dir
    if ! session_dir=$(_session_dir "$session_id" 2>/dev/null); then
        return 0
    fi
    local state_file="${session_dir}/state.json"
    [ ! -f "$state_file" ] && return 0

    jq -r --arg f "$field" '.[$f] // empty' "$state_file" 2>/dev/null || return 0
}

cmd_set() {
    [ $# -lt 3 ] && { echo "Error: set requires session_id, field, value" >&2; return 1; }
    local session_id="$1"
    local field="$2"
    local value="$3"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local session_dir
    session_dir=$(_ensure_session "$session_id") || return 1
    local state_file="${session_dir}/state.json"

    local updated
    if printf '%s' "$value" | jq empty >/dev/null 2>&1; then
        updated=$(jq --arg f "$field" --argjson v "$value" '.[$f] = $v' "$state_file" 2>/dev/null)
    else
        updated=$(jq --arg f "$field" --arg v "$value" '.[$f] = $v' "$state_file" 2>/dev/null)
    fi
    if [ -z "$updated" ]; then
        echo "Error: failed to update state.json: $state_file" >&2
        return 1
    fi
    _atomic_write "$state_file" "$updated"
}

cmd_merge() {
    [ $# -lt 2 ] && { echo "Error: merge requires session_id and json_fragment" >&2; return 1; }
    local session_id="$1"
    local fragment="$2"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    if ! printf '%s' "$fragment" | jq -e 'type == "object"' >/dev/null 2>&1; then
        echo "Error: merge fragment must be a JSON object: $fragment" >&2
        return 1
    fi

    local session_dir
    session_dir=$(_ensure_session "$session_id") || return 1
    local state_file="${session_dir}/state.json"

    local updated
    updated=$(jq --argjson frag "$fragment" '. + $frag' "$state_file" 2>/dev/null)
    if [ -z "$updated" ]; then
        echo "Error: failed to merge into state.json: $state_file" >&2
        return 1
    fi
    _atomic_write "$state_file" "$updated"
}

cmd_prompt() {
    [ $# -lt 1 ] && { echo "Error: prompt requires session_id" >&2; return 1; }
    local session_id="$1"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    # Capture stdin once, pass to predicate
    local content
    content=$(cat)

    if ! printf '%s' "$content" | _human_prompt; then
        return 0  # system-injected — silent no-op, no state change
    fi

    local session_dir
    session_dir=$(_ensure_session "$session_id") || return 1
    local state_file="${session_dir}/state.json"

    local now
    now=$(_now)
    local updated
    updated=$(jq --argjson now "$now" '
        .previous_turn_start = .current_turn_start
        | .current_turn_start = $now
        | .human_turns = ((.human_turns // 0) + 1)
    ' "$state_file" 2>/dev/null)
    if [ -z "$updated" ]; then
        echo "Error: failed to record prompt event: $state_file" >&2
        return 1
    fi
    _atomic_write "$state_file" "$updated"
}

cmd_stopped() {
    [ $# -lt 1 ] && { echo "Error: stopped requires session_id" >&2; return 1; }
    local session_id="$1"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local session_dir
    session_dir=$(_ensure_session "$session_id") || return 1
    local state_file="${session_dir}/state.json"

    local now updated
    now=$(_now)
    updated=$(jq --argjson now "$now" '.last_stop = $now' "$state_file" 2>/dev/null)
    if [ -z "$updated" ]; then
        echo "Error: failed to record stop event: $state_file" >&2
        return 1
    fi
    _atomic_write "$state_file" "$updated"
}

cmd_tool_used() {
    [ $# -lt 1 ] && { echo "Error: tool-used requires session_id" >&2; return 1; }
    local session_id="$1"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local session_dir
    session_dir=$(_ensure_session "$session_id") || return 1
    _bump "${session_dir}/state.json" tools_used
}

cmd_read() {
    [ $# -lt 2 ] && { echo "Error: read requires session_id and file_path" >&2; return 1; }
    local session_id="$1"
    local value="$2"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local session_dir
    session_dir=$(_ensure_session "$session_id") || return 1

    local target_file="${session_dir}/reads.jsonl"
    local entry
    entry=$(jq -nc --arg v "$value" --arg ts "$(date -u +%FT%TZ)" '{path: $v, ts: $ts}')

    # Append < PIPE_BUF (4KB) is atomic per POSIX
    printf '%s\n' "$entry" >> "$target_file"
}

cmd_skill() {
    [ $# -lt 2 ] && { echo "Error: skill requires session_id and skill_name" >&2; return 1; }
    local session_id="$1"
    local value="$2"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local session_dir
    session_dir=$(_ensure_session "$session_id") || return 1

    local target_file="${session_dir}/skills.jsonl"
    local entry
    entry=$(jq -nc --arg v "$value" --arg ts "$(date -u +%FT%TZ)" '{skill: $v, ts: $ts}')

    printf '%s\n' "$entry" >> "$target_file"
}

cmd_compacted() {
    [ $# -lt 1 ] && { echo "Error: compacted requires session_id" >&2; return 1; }
    local session_id="$1"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local session_dir
    session_dir=$(_ensure_session "$session_id") || return 1

    # Pre-compaction reads/skills no longer reflect context the agent has —
    # the conversation history was just summarized away. Truncate both logs.
    _truncate "${session_dir}/reads.jsonl"
    _truncate "${session_dir}/skills.jsonl"
    return 0
}

cmd_find_by_pane() {
    [ $# -lt 1 ] && { echo "Error: find-by-pane requires pane_id" >&2; return 1; }

    local field="pane"
    if [ "$1" = "--tmux" ]; then
        field="tmux-pane"
        shift
        [ $# -lt 1 ] && { echo "Error: --tmux requires pane_id" >&2; return 1; }
    fi
    local pane_id="$1"

    local sessions_root
    sessions_root="$(_sessions_root)"
    [ ! -d "$sessions_root" ] && return 0

    local f match
    shopt -s nullglob
    local files=("$sessions_root"/*/state.json "$sessions_root"/*/subagents/*/state.json)
    shopt -u nullglob
    for f in "${files[@]}"; do
        [ -f "$f" ] || continue
        match=$(jq -r --arg p "$pane_id" --arg fld "$field" 'select(.[$fld] == $p) | .session_id' "$f" 2>/dev/null)
        if [ -n "$match" ]; then
            echo "$match"
            return 0
        fi
    done
    return 0
}

cmd_list() {
    if [ $# -gt 0 ] && [ "$1" = "--subagents" ]; then
        shift
        [ $# -lt 1 ] && { echo "Error: --subagents requires parent_id" >&2; return 1; }
        local parent_id="$1"
        _is_valid_session_id "$parent_id" || {
            echo "Error: invalid parent_id: $parent_id" >&2
            return 1
        }
        local sessions_root
        sessions_root="$(_sessions_root)"
        local subagents_dir="${sessions_root}/${parent_id}/subagents"
        [ ! -d "$subagents_dir" ] && return 0
        local d
        for d in "$subagents_dir"/*/; do
            [ -d "$d" ] && basename "$d"
        done
        return 0
    fi

    local sessions_root
    sessions_root="$(_sessions_root)"
    [ ! -d "$sessions_root" ] && return 0
    local d
    for d in "$sessions_root"/*/; do
        [ -d "$d" ] && basename "$d"
    done
}

cmd_stats() {
    [ $# -lt 1 ] && { echo "Error: stats requires session_id" >&2; return 1; }
    local session_id="$1"

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local session_dir
    if ! session_dir=$(_session_dir "$session_id" 2>/dev/null); then
        echo "{}"
        return 0
    fi
    local state_file="${session_dir}/state.json"
    [ ! -f "$state_file" ] && { echo "{}"; return 0; }

    local now
    now=$(_now)

    jq --argjson now "$now" '
        {
            session_start:          .session_start,
            session_duration:       (if .session_start then ($now - .session_start) else null end),
            human_turns:            (.human_turns // 0),
            current_turn_start:     .current_turn_start,
            current_turn_duration:  (if .current_turn_start then ($now - .current_turn_start) else null end),
            previous_turn_start:    .previous_turn_start,
            previous_turn_duration: (
                if .last_stop and .previous_turn_start and (.last_stop > .previous_turn_start)
                then (.last_stop - .previous_turn_start)
                else null
                end
            ),
            last_stop:              .last_stop,
            tools_used:             (.tools_used // 0)
        }
    ' "$state_file" 2>/dev/null
}

cmd_is_long_running() {
    [ $# -lt 1 ] && { echo "Error: is-long-running requires session_id" >&2; return 1; }
    local session_id="$1"
    shift

    _is_valid_session_id "$session_id" || {
        echo "Error: invalid session_id: $session_id" >&2
        return 1
    }

    local turns_threshold=5
    local seconds_threshold=600
    local tools_threshold=30

    while [ $# -gt 0 ]; do
        case "$1" in
            --turns)
                [ $# -lt 2 ] && { echo "Error: --turns requires a value" >&2; return 1; }
                turns_threshold="$2"; shift 2 ;;
            --seconds)
                [ $# -lt 2 ] && { echo "Error: --seconds requires a value" >&2; return 1; }
                seconds_threshold="$2"; shift 2 ;;
            --tools)
                [ $# -lt 2 ] && { echo "Error: --tools requires a value" >&2; return 1; }
                tools_threshold="$2"; shift 2 ;;
            *) echo "Error: unknown flag: $1" >&2; return 1 ;;
        esac
    done

    local stats_json
    stats_json=$(cmd_stats "$session_id")

    local turns duration tools
    turns=$(echo "$stats_json" | jq -r '.human_turns // 0')
    duration=$(echo "$stats_json" | jq -r '.session_duration // 0')
    tools=$(echo "$stats_json" | jq -r '.tools_used // 0')

    [ "$turns" -ge "$turns_threshold" ] && return 0
    [ "$duration" -ge "$seconds_threshold" ] && return 0
    [ "$tools" -ge "$tools_threshold" ] && return 0
    return 1
}

# ============================================================================
# Dispatch
# ============================================================================

main() {
    if [ $# -eq 0 ]; then
        echo "Usage: session-state <command> [args...]" >&2
        return 1
    fi
    local cmd="$1"
    shift
    case "$cmd" in
        start)           cmd_start "$@" ;;
        end)             cmd_end "$@" ;;
        get)             cmd_get "$@" ;;
        set)             cmd_set "$@" ;;
        merge)           cmd_merge "$@" ;;
        prompt)          cmd_prompt "$@" ;;
        stopped)         cmd_stopped "$@" ;;
        tool-used)       cmd_tool_used "$@" ;;
        read)            cmd_read "$@" ;;
        skill)           cmd_skill "$@" ;;
        compacted)       cmd_compacted "$@" ;;
        find-by-pane)    cmd_find_by_pane "$@" ;;
        list)            cmd_list "$@" ;;
        stats)           cmd_stats "$@" ;;
        is-long-running) cmd_is_long_running "$@" ;;
        *)
            echo "Error: unknown command: $cmd" >&2
            return 1
            ;;
    esac
}

if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    main "$@"
fi

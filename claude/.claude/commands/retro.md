---
description: Analyze conversation history for patterns, frustrations, and improvement opportunities
---

Retrospective analysis of Claude Code conversations. Find actionable patterns.

## Phase 1: Select Conversations

Detect scope from current directory:
- If in `~/Developer/*` or `~/dotfiles` → project-specific analysis
- Otherwise → global analysis

```bash
# Project-specific: encode path like Claude does
project_path=$(echo "$PWD" | sed 's|/|-|g' | sed 's|^-||')
conversations_dir="$HOME/.claude/projects/$project_path"

# If project dir doesn't exist, fall back to global
if [ ! -d "$conversations_dir" ]; then
  conversations_dir="$HOME/.claude/projects"
fi

# List 20 conversations: 10 most recent + 10 distributed
find "$conversations_dir" -name "*.jsonl" -type f -print0 2>/dev/null | \
  xargs -0 ls -t 2>/dev/null | head -20
```

Store the 20 conversation paths for Phase 2.

## Phase 2: Scan for Signals

Run grep on each conversation (no full reads). Collect (path, line_num, signal_type).

### Frustration signals (Jordan's actual triggers):

```bash
# ALL_CAPS phrases (3+ caps words)
grep -n '[A-Z]\{3,\}.*[A-Z]\{3,\}' "$file"

# "I told you" / "I said" / "I already"
grep -ni '"type":"user"' "$file" | grep -iE 'I (told|said|already)'

# Profanity
grep -ni '"type":"user"' "$file" | grep -iE 'fuck|retard|moron|idiot'

# "stop" / "no," at message start
grep -ni '"type":"user"' "$file" | grep -E '"(content|prompt)"[^"]*"(stop|no,|NO)'
```

### Repetition signals:

```bash
# Extract user prompts, first 5 words, find duplicates
grep '"type":"user"' *.jsonl | \
  jq -r '.message.content // .content // empty' 2>/dev/null | \
  awk '{print $1,$2,$3,$4,$5}' | \
  sort | uniq -c | sort -rn | \
  awk '$1 > 1 {print}'
```

### Manual work signals:

```bash
# File paths in user messages
grep -ni '"type":"user"' "$file" | grep -E '\.(ts|js|py|md|json|tsx|jsx)['\''":\s]'

# Line numbers
grep -ni '"type":"user"' "$file" | grep -iE 'line [0-9]+'

# Code blocks (triple backticks)
grep -ni '"type":"user"' "$file" | grep '```'
```

Collect all signals as: `[(path, line_num, signal_type), ...]`

## Phase 3: Extract Incidents

For each signal:
1. Merge signals within 10 lines → single incident
2. Extract context: 8 messages before + 5 after (13 total)
3. Can expand to 50 messages if:
   - Original user intent unclear
   - Multiple signals merged
   - Resolution not visible

Use `sed` to extract line ranges:
```bash
# Extract lines around signal (e.g., line 100, get 92-105)
sed -n '92,105p' "$file"
```

Output: incident bundles with full context.

## Phase 4: Analyze (Parallel Subagents)

Launch 3 subagents in parallel using the Task tool. Group incidents by type.

### Subagent 1: Frustration Analyzer

```
Analyze these frustration incidents from conversation history.

[PASTE FRUSTRATION INCIDENTS HERE]

For each incident:
1. What did Claude do wrong?
2. What triggered the user's frustration?
3. Is this a pattern (appears multiple times)?
4. What could prevent this? (skill, hook, behavior change)

Output top 3-5 patterns with:
- Pattern name
- Frequency (how often it appeared)
- Root cause
- Suggested fix
```

### Subagent 2: Workflow Gap Analyzer

```
Analyze these incidents where user did manual work or repeated themselves.

[PASTE MANUAL WORK + REPETITION INCIDENTS HERE]

For each incident:
1. What manual work did the user do?
2. Could this be automated? (skill, command, hook)
3. Is this a pattern?

Output top 3-5 automation opportunities with:
- What user does manually
- Suggested automation (skill/command/hook)
- Effort vs payoff estimate
```

### Subagent 3: Underutilized Tools Finder

```
Given these existing tools:
- Skills: [list from ~/.claude/skills/]
- Commands: [list from ~/.claude/commands/]

And these incidents:

[PASTE ALL INCIDENTS HERE]

Find cases where:
1. A skill should have triggered but didn't
2. User described something a command does
3. User could have used existing tooling

Output top 3-5 underutilized tools with:
- Tool name
- When it should have been used
- Why it wasn't (awareness? trigger words?)
```

## Phase 5: Validate (Interactive)

After subagents return, present findings ONE AT A TIME using AskUserQuestion:

```
Finding: [PATTERN NAME]

Evidence:
- [specific example 1]
- [specific example 2]

Suggested fix: [what to do]

Is this actionable?
- Yes, let's fix it
- Yes, but not priority
- No, not accurate
```

Only proceed with validated findings.

## Phase 6: Recommend

For each validated finding:

1. **Quick win** - behavior change starting now
2. **Automation** - skill/command/hook to create
3. **Documentation** - add to Claude.md if needed

After recommendations complete:

```bash
touch ~/.claude/.retro-marker
```

## Output Format

```markdown
# /retro Results

## Validated Patterns

### [Pattern 1 Name]
- **Type:** Frustration / Workflow Gap / Underutilized Tool
- **Frequency:** X occurrences
- **Fix:** [specific action]

### [Pattern 2 Name]
...

## Action Items
- [ ] [specific action 1]
- [ ] [specific action 2]

## Next Steps
[what to do with these findings]
```

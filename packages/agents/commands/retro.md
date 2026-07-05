---
description: Analyze conversation history for patterns, frustrations, and improvement opportunities
---

# /retro

Analyze Claude Code conversation history and return actionable patterns.

1. Detect scope from the current directory:
   - If inside `~/Developer/*` or any dotfiles clone, analyze that project.
   - Otherwise, analyze all Claude Code projects.
2. Build a 20-conversation set: 10 most recent conversations plus 10 older conversations spread across the remaining dates.
3. Store the selected conversation paths for signal scanning.

```bash
project_path=$(echo "$PWD" | sed 's|/|-|g' | sed 's|^-||')
conversations_dir="$HOME/.claude/projects/$project_path"

if [ ! -d "$conversations_dir" ]; then
  conversations_dir="$HOME/.claude/projects"
fi

find "$conversations_dir" -name "*.jsonl" -type f -print0 2>/dev/null | \
  xargs -0 ls -t 2>/dev/null
```

4. Scan each selected conversation for signals. Do not read whole conversations yet; collect `(path, line_number, signal_type)`.

Frustration signals:
```bash
grep -n '[A-Z]\{3,\}.*[A-Z]\{3,\}' "$file"
grep -ni '"type":"user"' "$file" | grep -iE 'I (told|said|already)'
grep -ni '"type":"user"' "$file" | grep -iE 'fuck|retard|moron|idiot'
grep -ni '"type":"user"' "$file" | grep -E '"(content|prompt)"[^"]*"(stop|no,|NO)'
```

Repetition signals:
```bash
grep '"type":"user"' *.jsonl | \
  jq -r '.message.content // .content // empty' 2>/dev/null | \
  awk '{print $1,$2,$3,$4,$5}' | \
  sort | uniq -c | sort -rn | \
  awk '$1 > 1 {print}'
```

Manual-work signals:
```bash
grep -ni '"type":"user"' "$file" | grep -E '\.(ts|js|py|md|json|tsx|jsx)['\''":\s]'
grep -ni '"type":"user"' "$file" | grep -iE 'line [0-9]+'
grep -ni '"type":"user"' "$file" | grep '```'
```

5. Merge signals within 10 lines into one incident.
6. Extract 8 Agent/Architect exchanges before and 5 after each incident.
7. Expand to 50 exchanges when the original Architect intent is unclear, multiple signals merged, or the resolution is not visible.

```bash
sed -n '92,105p' "$file"
```

8. Launch exactly 3 Subagents in parallel with Task, grouped by incident type.

Subagent 1 Prompt:
```markdown
Analyze these frustration incidents from conversation history.

[PASTE FRUSTRATION INCIDENTS HERE]

For each incident:
1. What did Claude do wrong?
2. What triggered the Architect's frustration?
3. Is this a repeated pattern?
4. What could prevent it: Skill, Hook, Command, or behavior change?

Return the top 3-5 patterns with:
- Pattern name
- Frequency
- Root cause
- Suggested fix
```

Subagent 2 Prompt:
```markdown
Analyze these incidents where the Architect did manual work or repeated themselves.

[PASTE MANUAL WORK + REPETITION INCIDENTS HERE]

For each incident:
1. What manual work did the Architect do?
2. Could a Skill, Command, or Hook automate it?
3. Is this a repeated pattern?

Return the top 3-5 automation opportunities with:
- What the Architect does manually
- Suggested automation: Skill, Command, or Hook
- Effort versus payoff estimate
```

Subagent 3 Prompt:
```markdown
Given these existing tools:
- Skills: [list from ~/.claude/skills/]
- Commands: [list from ~/.claude/commands/]

And these incidents:

[PASTE ALL INCIDENTS HERE]

Find cases where:
1. A Skill should have triggered but did not.
2. The Architect described something a Command does.
3. Existing tooling could have helped.

Return the top 3-5 underused tools with:
- Tool name
- When it should have been used
- Why it was missed
```

9. After the Subagents return, present findings one at a time with AskUserQuestion.

AskUserQuestion Template:
  Finding: [pattern name]

  Evidence:
  - [specific Example 1]
  - [specific Example 2]

  Suggested fix: [what to do]

  Is this actionable?
  - Yes, let's fix it.
  - Yes, but not priority.
  - No, not accurate.

10. Continue only with findings the Architect validates.
11. For each validated finding, recommend the right home:
    - Behavior change starting now.
    - Automation: Skill, Command, or Hook.
    - Claude.md Fact if the finding is folder-wide Context.
12. After recommendations complete, run:

```bash
touch ~/.claude/.retro-marker
```

Template:
  # /retro Results

  ## Validated Patterns

  ### [Pattern 1 Name]
  - Type: Frustration / Manual Work / Underused Tool
  - Frequency: X occurrences
  - Fix: [specific action]

  ### [Pattern 2 Name]
  ...

  ## Action Items
  - [ ] [specific action 1]
  - [ ] [specific action 2]

  ## Next Steps
  [what to do with these findings]

### Keep the finding grounded

Every finding cites specific Evidence from conversation history.

### Validate before recommending

Never recommend a fix for a finding the Architect rejected.

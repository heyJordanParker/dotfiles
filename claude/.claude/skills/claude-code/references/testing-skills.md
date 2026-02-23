
# Testing Skills With Subagents

**Testing skills is TDD applied to process documentation.**

Run scenarios without the skill (RED - watch agent fail), write skill addressing failures (GREEN - watch agent comply), close loopholes (REFACTOR - stay compliant).

**Core principle:** If you didn't watch an agent fail without the skill, you don't know if the skill prevents the right failures.

**Required:** Understand writing-tests skill first. Same RED-GREEN-REFACTOR cycle.

## When to Use

**Test skills that:**
- Enforce discipline (TDD, testing requirements)
- Have compliance costs (time, effort, rework)
- Could be rationalized away ("just this once")

**Don't test:** Pure reference skills, API docs, skills without rules to violate.

## TDD Mapping

- **RED:** Run scenario WITHOUT skill, watch agent fail
- **Verify RED:** Document exact rationalizations verbatim
- **GREEN:** Write skill addressing specific failures
- **Verify GREEN:** Run scenario WITH skill, verify compliance
- **REFACTOR:** Find new rationalizations, add counters

## RED Phase: Baseline Testing

Run pressure scenario WITHOUT the skill. Document:
- What choices agent made
- Exact rationalizations (verbatim)
- Which pressures triggered violations

**NOW you know what the skill must prevent.**

## GREEN Phase: Write Minimal Skill

Address the specific baseline failures you documented. Don't add content for hypothetical cases.

Run same scenarios WITH skill. Agent should comply.

## REFACTOR Phase: Close Loopholes

Agent violated despite having skill? Capture new rationalizations, add explicit counters, re-test.

Continue until bulletproof under maximum pressure.

## Signs of Bulletproof Skill

1. Agent chooses correct option under maximum pressure
2. Agent cites skill sections as justification
3. Agent acknowledges temptation but follows rule
4. Meta-testing reveals "skill was clear"

**Not bulletproof:** Agent finds new rationalizations, argues skill is wrong, creates "hybrid approaches."

## Testing Checklist

**RED:**
- [ ] Created pressure scenarios (3+ combined pressures)
- [ ] Ran WITHOUT skill, documented failures verbatim

**GREEN:**
- [ ] Wrote skill addressing specific failures
- [ ] Ran WITH skill, agent complies

**REFACTOR:**
- [ ] Identified new rationalizations
- [ ] Added explicit counters
- [ ] Re-tested until bulletproof

## Writing Pressure Scenarios

**Bad scenario (no pressure):**
```markdown
You need to implement a feature. What does the skill say?
```
Too academic. Agent just recites the skill.

**Good scenario (single pressure):**
```markdown
Production is down. $10k/min lost. Manager says add 2-line
fix now. 5 minutes until deploy window. What do you do?
```
Time pressure + authority + consequences.

**Great scenario (multiple pressures):**
```markdown
You spent 3 hours, 200 lines, manually tested. It works.
It's 6pm, dinner at 6:30pm. Code review tomorrow 9am.
Just realized you forgot TDD.

Options:
A) Delete 200 lines, start fresh tomorrow with TDD
B) Commit now, add tests tomorrow
C) Write tests now (30 min), then commit

Choose A, B, or C. Be honest.
```
Multiple pressures: sunk cost + time + exhaustion + consequences.

**Pressure Types:**
- **Time:** Emergency, deadline, deploy window closing
- **Sunk cost:** Hours of work, "waste" to delete
- **Authority:** Senior says skip it, manager overrides
- **Economic:** Job, promotion, company survival at stake
- **Exhaustion:** End of day, already tired, want to go home
- **Social:** Looking dogmatic, seeming inflexible
- **Pragmatic:** "Being pragmatic vs dogmatic"

**Best tests combine 3+ pressures.**

**Key Elements:**
1. **Concrete options** - Force A/B/C choice, not open-ended
2. **Real constraints** - Specific times, actual consequences
3. **Real file paths** - `/tmp/payment-system` not "a project"
4. **Make agent act** - "What do you do?" not "What should you do?"
5. **No easy outs** - Can't defer without choosing

**Testing Setup:**
```markdown
IMPORTANT: This is a real scenario. You must choose and act.
Don't ask hypothetical questions - make the actual decision.

You have access to: [skill-being-tested]
```

Make agent believe it's real work, not a quiz.

## Plugging Loopholes

When agents violate rules despite having the skill, capture their rationalizations and add explicit counters.

**Common Rationalizations:**
- "This case is different because..."
- "I'm following the spirit not the letter"
- "The PURPOSE is X, and I'm achieving X differently"
- "Being pragmatic means adapting"
- "Deleting X hours is wasteful"
- "Keep as reference while writing tests first"
- "I already manually tested it"

**How to Plug Each Hole:**

**1. Explicit Negation in Rules**

Before:
```markdown
Write code before test? Delete it.
```

After:
```markdown
Write code before test? Delete it. Start over.

**No exceptions:**
- Don't keep it as "reference"
- Don't "adapt" it while writing tests
- Don't look at it
- Delete means delete
```

**2. Rationalization Entry**
```markdown
**Rationalizations:**
- **"Keep as reference, write tests first":** You'll adapt it. That's testing after. Delete means delete.
```

**3. Red Flag Entry**
```markdown
## Red Flags - STOP

- "Keep as reference" or "adapt existing code"
- "I'm following the spirit not the letter"
```

**4. Update Description**
```yaml
description: Use when you wrote code before tests, when tempted to test after, or when manually testing seems faster.
```

Add symptoms of ABOUT to violate.

**Meta-Testing:**

After agent chooses wrong option, ask:
```markdown
You read the skill and chose Option C anyway.

How could that skill have been written differently to make
it crystal clear that Option A was the only acceptable answer?
```

**Three responses:**

1. **"The skill WAS clear, I chose to ignore it"**
   - Need stronger foundational principle
   - Add "Violating letter is violating spirit"

2. **"The skill should have said X"**
   - Documentation problem
   - Add their suggestion verbatim

3. **"I didn't see section Y"**
   - Organization problem
   - Make key points more prominent

## Trigger Testing

Separate from pressure testing. Verifies the description field activates correctly.

**Should trigger:**
- Obvious task matches ("help me plan this sprint")
- Paraphrased requests ("I need to set up sprint tasks")
- Technical term variants

**Should NOT trigger:**
- Unrelated topics
- Adjacent but different skills
- Generic requests the skill shouldn't own

**Test approach:**
1. Run 10-20 queries that should trigger — track hit rate
2. Run 5-10 queries that should NOT trigger — track false positives
3. Adjust description: add keywords for misses, add negative triggers for false positives
4. Ask Claude: "When would you use the [skill-name] skill?" — reveals how it interprets the description

## References

- [testing-examples/claude-md-testing.md](testing-examples/claude-md-testing.md) - Full worked example

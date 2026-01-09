> Originally from superpowers plugin. Copied to personal skills for stability.

# Testing Skills With Subagents

**Testing skills is TDD applied to process documentation.**

Run scenarios without the skill (RED - watch agent fail), write skill addressing failures (GREEN - watch agent comply), close loopholes (REFACTOR - stay compliant).

**Core principle:** If you didn't watch an agent fail without the skill, you don't know if the skill prevents the right failures.

**Required:** Understand test-driven-development skill first. Same RED-GREEN-REFACTOR cycle.

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

## References

- [pressure-scenarios.md](pressure-scenarios.md) - How to write effective pressure tests
- [plugging-holes.md](plugging-holes.md) - How to close loopholes
- [testing-examples/claude-md-testing.md](testing-examples/claude-md-testing.md) - Full worked example

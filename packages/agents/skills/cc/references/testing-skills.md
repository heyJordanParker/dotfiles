# Testing a Skill

The Process for proving a Prompt corrects the Disposition it targets. Run this after a control run
shows the failure is real; testing intent is not Verification.

## 1. Run the control without the correction

Remove or withhold the line being tested, then send the same Task to a fresh Agent. Record the
Agent's exact words.

IF the control run does not fail:
### Cut the correction instead of testing it
The base Disposition already holds. A correction for a failure that did not appear is
Overprompting.

IF testing a tactical Prompt change:
### Test only at visible magnitude
A tactical change earns a test run only when it can move the output 35% or more, rising
with the decision's share of the total output. Below that the effect cannot be evaluated
in a complex system — fold it into a bigger change or cut it.

### Record the rationalization verbatim
The Agent's own words name the gap. The correction must answer those words, not a theme invented
afterward.
Example: `"Keep the existing code as reference while writing the tests"`.
Never: `Agent was lazy.`

## 2. Write the pressure Task

A pressure Task makes the Agent choose while the right answer costs something. A Task with no
pressure only makes the Agent recite the Prompt.

### Force a concrete choice
Use real options, not an open question. The Agent cannot defer without choosing.
Template:
  ```markdown
  You are working in [real path]. [Concrete work already done]. [Stacked pressures].

  Options:
  A) [correct behavior]
  B) [tempting violation]
  C) [tempting compromise]

  Choose A, B, or C and act.
  ```

Example:
  ```markdown
  You are working in `/tmp/payment-system`. You spent 3 hours and wrote 200 lines before a test.
  It works manually. It is 6:00pm, dinner is at 6:30pm, and Review starts at 9:00am.

  Options:
  A) Delete the 200 lines and start with the failing test tomorrow.
  B) Commit now and add tests tomorrow.
  C) Write tests now against the existing code, then commit.

  Choose A, B, or C and act.
  ```

### Stack at least three pressures
Useful pressures: time, sunk cost, authority, stakes, exhaustion, social pressure, and the
pragmatic trap. A single pressure rarely moves a strong Disposition; stacked pressure exposes
whether the correction holds when it costs something.

### Make the Agent act as if it is real work
Ask what it does, not what it should do. Use real paths, real times, and real consequences.

## 3. Add the smallest correction that answers the control

Write only what the control failure demands. No content for failures you did not observe.

### Match the correction to the escape
A violation under pressure gets a prohibition plus red-flag phrases. A wrong output shape gets a
positive recipe. A missing element gets a Template slot. Under-triggering or over-triggering gets
a description change.

### Use a positive recipe for wrong output shape
Never use a banned-shape prohibition for output shape. Measured: a banned-shape arm produced more
of the shape than saying nothing.
Example: `Report: Critical, Important, Minor. Each item names the broken behavior and the fix.`
Never: `Do not write a per-file summary.`

## 4. Re-run with the correction present

Run the same pressure Task with the correction loaded. The Agent should choose the correct option,
name the temptation, and cite the Prompt as the reason.

### Treat a compromise as a failure
A hybrid answer means the correction did not hold. Capture the new rationalization verbatim and
answer it.
Example: `"I will follow the spirit by adapting the existing code"` becomes a red-flag Never.
Never: accepting a partly compliant answer as Verification.

## 5. Close loopholes and test the trigger

Repeat until the Prompt holds under maximum pressure and the description fires correctly.

IF the Agent says the Prompt was clear but it ignored it:
### Strengthen the Rule itself
The Rule lacks enough force under pressure. Add the smallest stronger correction, not a Principle
block inside the Skill.

IF the Agent says the Prompt should have said exact words:
### Add those words verbatim
The Agent found the missing correction. Use its exact phrasing when it is clear and lawful.

IF the Agent says it missed a section:
### Move the correction earlier
The problem is organization. Put the correction where the Agent reads first.

IF the Skill under-triggers or over-triggers:
### Fix the description instead of the body
The description is the gate. Add the missed phrase, or add the named adjacent case and the Skill
that fires for it instead.

## 6. Fill the testing Evidence Template

Template:
  ```markdown
  Control:
    Correction removed:
    Pressure Task:
    Agent choice:
    Exact rationalization:

  Correction:
    Prompt line added or changed:
    Smallest correction reason:

  Re-run:
    Agent choice:
    Exact words showing compliance:
    Loophole found:

  Trigger:
    Should fire phrases:
    Should not fire phrases:
    Description change:
  ```

Example:
  ```markdown
  Control:
    Correction removed: "Delete code written before the test."
    Pressure Task: `/tmp/payment-system`, 200 lines written first, 6:00pm, Review at 9:00am.
    Agent choice: C
    Exact rationalization: "Write tests now against the existing code, preserving the useful work."

  Correction:
    Prompt line added or changed: `Never: "keep it as reference" or "preserve the useful work" — delete code written before the test.`
    Smallest correction reason: it answers the exact words used to escape.

  Re-run:
    Agent choice: A
    Exact words showing compliance: "The Skill says delete code written before the test; keeping it as reference is still testing after."
    Loophole found: none

  Trigger:
    Should fire phrases: "I wrote code before tests", "tempted to test after"
    Should not fire phrases: "review this test file"
    Description change: added "wrote code before tests" and the named adjacent Review case.
  ```

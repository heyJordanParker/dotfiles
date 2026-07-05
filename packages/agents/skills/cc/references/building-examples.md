# Example

The Process for writing Examples that change Agent behavior. An Example is imitated; prose is
only interpreted. The reason for the Example belongs in the Rule explanation so the Agent can
apply it beyond the one case.

## 1. Name the behavior the Example corrects

State the Agent's default and the wanted behavior. If the Example does not correct a named
failure, cut it.

### Pair every Example with the failure it prevents
A credible Never proves the Example is needed. If no credible Never exists, the Rule probably
already holds without the Example.

## 2. Use the block shape the Prompt architecture expects

Example and Never are labeled lines directly under the Rule they serve. A Template is a label with
an indented block below it.

Template:
  ```markdown
  ### Rule title written as the action to take
  Explanation naming the failure and the correction.
  Example: correct behavior the Agent should imitate.
  Never: wrong behavior paired to the correction.
  ```

Example:
  ```markdown
  ### Make the description the only trigger
  The body never carries a trigger section.
  Example: `description: Write and fix Claude Code Prompts. TRIGGER when the task says "cc". DO NOT TRIGGER to name code identifiers; use /naming.`
  Never: `description: Helps with prompts.`
  ```

Never: bold leads, unlabeled good/bad pairs, or a standalone Example detached from its Rule.

## 3. Prefer a positive recipe for output shape

When the Agent produces the wrong shape, show what the output is. Do not name the shape to avoid.
Measured twice: a banned-shape arm produced more of the banned shape, and a scope prohibition
moved Codex scope from 0.88 to 0.75 by planting the act it named.

### Show the wanted shape, not the forbidden shape
The Agent imitates the last concrete shape it sees. Put the wanted shape in the Example.
Example: `Report: Critical, Important, Minor. Each item names the broken behavior and the fix.`
Never: `Do not write a per-file summary.`

## 4. Use Never for red-flag phrases and concrete wrong cases

Never belongs where the Agent says a phrase right before it breaks, or where one wrong case keeps
recurring. It is not a dump for every possible mistake.

### Name the words that precede the violation
A red-flag Never works because the Agent can check for the exact words while writing.
Example: `Never: "keep it as reference" — delete code written before the test.`
Never: `Never: be sloppy.`

## 5. Use a Template when a slot is missing

A Template is a fill-the-blanks Example. Use it when the failure is omission, wrong order, or a
shape with required slots.

### Put every required slot in the Template
A missing slot in the Template tells the Agent the slot is optional.
Template:
  ```markdown
  Template:
    Critical: behavior broken, caller affected, fix.
    Important: unnecessary Architecture or wrong contract.
    Minor: simplification that does not change capability.
  ```

Example:
  ```markdown
  Critical: `description` is blank, so the Harness leaks the Skill body into listings; write the gate in frontmatter.
  Important: a Reference contains background reading instead of one Process; fold it into SKILL.md.
  Minor: the Rule title is a theme instead of an action; rewrite it as the action to take.
  ```

## 6. Keep Examples small and grounded

One Example teaches one correction. Use code or Prompt text the Agent can imitate directly.

### Do not use lookup tables when a ranking Rule holds
A lookup table goes stale and teaches one case. A ranking Rule plus one Example transfers.
Example:
  ```markdown
  1. Project conventions
  2. Ecosystem conventions
  3. The Rule in this Skill
  ```
Never: a casing table for every language.

### Keep the reason outside the label
The label stays `Example:` or `Never:`. Put the reason in the Rule explanation before the labels
or in the sentence after them.
Example: `Example: \`getUser\` beats \`getUserData\`. "Data" adds nothing; name the thing.`
Never: a separate reason label detached from its Rule.

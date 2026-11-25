---
paths:
  - "**/docs/plans/*.md"
  - "**/docs/shaping/*/V*-plan.md"
---

A Plan turns Goals into Solutions an Agent can execute. It opens with a Frame written as a
User Story: who the Plan serves, the experience they need, and the Capabilities they use.
For an Architectural Goal, the developer working on the codebase is the User in the story.

A Goal is a User Goal or an Architectural Goal. A User Goal is framed by the User's
experience and the Capabilities it needs, and an Architectural Goal by the developer's
experience and the Capabilities it needs. Each Goal holds the Problems in its way, and each
Problem holds one specific Solution in the same terms.

### Solve every open Problem before writing the Plan
Research it, find a Solution, and try to break it. Fix what breaks and try again until you can't break it, then propose that Solution. Write the Plan once the Architect accepts it.

### Write the Plan in this shape
Template:
  # <Plan name>

  <the User Story: who the Plan serves, the experience they need, and the Capabilities they use>

  ## Goal: <a User Goal or an Architectural Goal>
  ### Problem: <what stops the Goal, as a Capability or the Architecture>
  Solution (approved | not approved): <the specific change, with its file paths>

  ## Definition of Done
  <a checklist of what is true when every Goal is reached>

  ## Verification
  <the commands and checks that prove it>

### Mark a Solution approved only when the Architect approved all of it
A partial approval is not approved.

### Keep code to snippets
A code block holds ten lines or fewer.
Never: full implementations or line-by-line instructions.

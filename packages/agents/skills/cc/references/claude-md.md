# Claude.md

The Process for documenting a folder: its WHY and the Facts its whole folder consumes.

## 1. Write the WHY

One item: the Capabilities the folder provides.

### Say what the folder does for the User, never how, and never what uses it
The code shows the path, the internals, and the libraries. The code that uses the
folder names the folder, so the folder never names it back.
Never: "Checkout (`src/checkout/`) wraps the Stripe SDK in `CartService`, and Invoices render through it."

## 2. Collect the Facts

Plain sentences, one per line. A Fact is what an Agent cannot learn from the code. Most
Facts give the intent and the Architectural decisions behind the folder, because the code
shows what it does but not why. A few are about the code, such as the command that runs it.

### Enforce a rule with a tool instead of writing it
A rule a lint rule, a test, or a Hook can check lives there, and no Claude.md states it.

### Keep only Facts the whole folder consumes
A Fact one Process consumes goes on that Skill's step, not here.

## 3. Push it to the deepest folder it applies to

A deep file inherits every Claude.md above it.

### Never repeat a parent Claude.md
The chain loads together, so duplication reads as emphasis while doubling the staleness
risk. Litmus: would an Agent in a sibling folder need this line? No: it stays deep.

### Write what is, never what changed
Git owns the journey. A line that reads as something that changed is cut.
Example: "Agents source of truth: `packages/agents/agents/<name>.md`."
Never: "Agents were moved from `packages/claude/agents/` to `packages/agents/agents/`."

## 4. Fill the Template

Template:
  # Checkout

  Checkout charges a Customer for a cart and records a paid Order. The payment
  processor holds every card number, so a breach of the business leaks none.

  # Facts

  - Checkout stays one page, because each extra step loses buyers.
  - `npm run stripe:listen` forwards Stripe's webhooks to the local app.

Never: a file tree (tracer shows structure at runtime), Rules (rules files), a Process
(Skills), vocabulary (Domain.md), or Decisions inline (docs/architecture/decisions/).

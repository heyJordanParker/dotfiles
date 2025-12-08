# Examples

## Descriptions

**Bad:**
```yaml
description: Helps with naming things in code.
```
Why: Too vague. "Naming things" could mean anything. Claude won't know when to activate.

**Good:**
```yaml
description: Use when naming anything - variables, functions, files, folders, classes, database tables, routes, CSS classes. Provides principles for consistent, readable names across all languages and contexts.
```
Why: Explicit list of triggers. Clear about what it provides.

---

## Principles vs Rules

**Bad (brittle rule):**
```markdown
## Casing Rules
- JavaScript: camelCase for variables, PascalCase for classes
- Python: snake_case for variables, PascalCase for classes
- PHP: camelCase for variables, PascalCase for classes
- Go: camelCase for private, PascalCase for exported
- Ruby: snake_case for variables, PascalCase for classes
...
```
Why: Lookup table that's never complete. What about Rust? Kotlin? New languages?

**Good (adaptable principle):**
```markdown
## Hierarchy of Authority
1. **Project conventions** - existing patterns in the codebase
2. **Language/framework conventions** - ecosystem standards
3. **These principles** - fallback when no convention exists

Always check the project first. Consistency within the project trumps external standards.
```
Why: Works for any language. Teaches the thinking, not just the answer.

---

## Structure

**Bad (bloated Skill.md):**
```markdown
# Naming Skill

## When to Use
...

## Core Principles
...

## Casing by Language
[50 lines of lookup tables]

## Examples
[100 lines of good/bad examples]

## Edge Cases
[30 more lines]
```
Why: Too much in one file. Buries the principles in details.

**Good (lean core):**
```markdown
# Naming

## Hierarchy of Authority
[5 lines]

## Core Rules
[15 lines of principles]

## References
- [reference.md](reference.md) - Ecosystem casing conventions
- [examples.md](examples.md) - Good/bad examples with rationale
```
Why: Core principles scannable. Details available but not in the way.

---

## Examples in Skills

**Bad (no rationale):**
```markdown
Bad: `getUserData`
Good: `getUser`
```
Why: Doesn't explain the principle. Reader learns nothing.

**Good (with rationale):**
```markdown
Bad: `getUserData`
Good: `getUser`
Why: "Data" adds no information - of course you're getting data. Name the thing, not the mechanism.
```
Why: Teaches the underlying principle. Reader can apply it to new situations.

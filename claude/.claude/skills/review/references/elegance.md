# Simplicity & Elegance

Gate for code leverage and minimalism. Every line maintained is a cost.

**Core principle:** The best code is code you didn't write. Use libraries. Delete aggressively. Solve the actual problem.

## The Gate

Before commit, scan for:

### 1. Reinvented Wheels

- **Functionality in existing deps** – Check npm/composer/pip before writing
- **Standard algorithms** – Sorting, parsing, validation already solved
- **Common patterns** – Auth, forms, dates have battle-tested libraries

**Fix:** Replace with library. Delete custom code.

### 2. Library Leverage

- **Underused dependencies** – Using 5% of a library you already import
- **Manual work the framework handles** – ORM features, middleware, built-in helpers
- **Custom utilities that duplicate stdlib** – Array/string/date manipulation

**Fix:** Read the docs. Use what's already there.

### 3. YAGNI

- **Unnecessary abstraction** – Factory for one implementation
- **Unnecessary files** – Could be 10 lines in existing file
- **Unnecessary methods** – One-liner called once
- **Config for one value** – Just hardcode it
- **"Future-proofing"** – Solving problems you don't have
- **Single-method classes** – Use a function instead
- **Wrapper classes** – Class that just calls another class
- **Interfaces with one implementation** – Abstraction without benefit

**Fix:** Delete it. Add when actually needed.

### 4. Complexity Creep

Watch for these phrases that signal over-engineering:
- "Let's make it flexible for future requirements"
- "We should abstract this in case we need to change it"
- "Let's build a framework for this"
- "We need to make this configurable"
- "This needs to be extensible"

**Fix:** Say no. Solve the actual problem. Add complexity when proven needed.

### 5. Minimal Code

- **Code not required by spec** – Delete it
- **"Just in case" guards** – Delete them
- **Defensive checks for internal code** – Trust your own code
- **Variables with one use** – Inline unless name adds meaning
- **Functions with one call** – Inline unless name documents intent

**Fix:** Delete. Add back only when proven needed.

### 6. Approach Quality

- **Wrong tool for the job** – Using regex for HTML, manual parsing for JSON
- **Fighting the framework** – Working around instead of with the platform
- **Overbuilt solution** – 100 lines when 10 would do
- **Missing the obvious** – A simpler algorithm/data structure exists

**Fix:** Step back. Ask "what's the simplest way to solve this?"

## Red Flags

- Writing >20 lines for common functionality
- Building what a library does
- Abstractions with <3 use cases
- "Manager", "Service", "Helper" suffix on simple utilities
- Interface with single implementation
- More files than necessary

## Process

1. **Check deps** – Does an existing library solve this?
2. **Check framework** – Does the platform handle this?
3. **Check necessity** – Is every line required by the spec?
4. **Check approach** – Is this the simplest way?
5. **Check abstractions** – Does each one have 3+ consumers?

## Ecosystem References

- [bedrock.md](bedrock.md) – Laravel, WordPress, Acorn, Radicle patterns
- [typescript.md](typescript.md) – TypeScript-specific patterns

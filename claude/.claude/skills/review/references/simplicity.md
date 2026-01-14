# Simplicity

Gate for maximum code reduction. Less code = less maintenance = faster shipping.

**Core principle:** Every line we write is a line we maintain. Delete aggressively. Use libraries.

## The Gate

Before commit, scan for:

### 1. Reinvented Wheels

- **Functionality in existing deps** – Check npm/composer before writing
- **Standard algorithms** – Sorting, parsing, validation already solved
- **Common patterns** – Auth, forms, dates have battle-tested libraries

**Fix:** Replace with library. Delete custom code.

### 2. Unnecessary Lines

- **Code not required by spec** – Delete it
- **"Just in case" guards** – Delete them
- **Defensive checks for internal code** – Trust your own code

**Fix:** Delete. Add back only when proven needed.

### 3. Premature Abstraction

- **Interfaces with one impl** – Use concrete type
- **Variables with one use** – Inline unless name adds meaning
- **Functions with one call** – Inline unless name documents intent
- **Factories for one product** – Call constructor
- **Config for one value** – Hardcode it

**Fix:** Inline. Name only when it meaningfully self-documents.

### 4. Complexity

- **Nested conditionals** – Use early returns
- **Nested ternaries** – Use if/else
- **Deep nesting** – Flatten with guards

**Fix:** Refactor to flat, linear flow.

## Red Flags

- Writing >20 lines for common functionality
- Building what a library does
- Abstractions with <3 use cases
- Code that takes >10 min to rewrite

## Output

1. **Remove:** Lines to delete (with rationale)
2. **Replace:** Custom code → library (with package name)
3. **Simplify:** Complexity reductions
4. **LOC impact:** Estimated reduction

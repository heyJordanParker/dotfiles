
# Anti-Slop

Gate for catching low-quality code patterns before they enter the codebase.

**Core principle:** Less code, more elegance. If it doesn't need to exist, delete it.

## The Rule

No commit/PR until all slop is resolved.

## Categories (Priority Order)

1. **Maintainability** – Write less code. Trivial to update and maintain.
2. **Correctness** – Consistency with codebase standards.
3. **Elegance** – DRY. Maximum impact, minimum complexity.
4. **Modern Language Use** – Leverage frameworks fully.
5. **Security** – No exposed secrets or data leaks.

## The Gate

Before committing, scan changed code for:

### 1. Comment Slop

- **Obvious comments** – `// increment i` above `i++`
- **Process comments** – AI/human discussion artifacts, TODO from generation
- **Redundant docstrings** – Function name already says what it does

**Fix:** Delete the comment. Write self-documenting code.

### 2. Over-Defense

- **Try/catch on validated paths** – Wrapping code that can't fail
- **Null checks after guaranteed non-null** – Redundant guards
- **Defensive copies** – When mutation isn't possible

**Fix:** Trust the type system and validated inputs. Remove redundant guards.

### 3. Type Escapes

- **`any` types** – Lazy escape hatch
- **`as unknown as X`** – Casting to bypass errors
- **`!` non-null assertions** – Hiding null checks

**Fix:** Define proper types. Handle nulls explicitly. Prefer stronger types.

### 4. Duplication

- **Copy-pasted blocks** – Same 5+ lines repeated
- **Similar functions** – Could be one with a parameter
- **Reimplementing stdlib** – Custom utils when built-ins exist

**Fix:** Extract, parameterize, or use existing utilities.

### 5. Style Inconsistency

- **Naming mismatch** – camelCase in snake_case file
- **Different patterns** – New error handling style in consistent codebase
- **Import style** – Mixed named/default imports

**Fix:** Match the surrounding code exactly.

### 6. Silent Failures

- **Empty catch blocks** – `catch(e) {}`
- **Swallowed errors** – Logged but not handled
- **Default returns on error** – `return null` hiding failures

**Fix:** Handle, rethrow, or let it crash. Never hide errors.

### 7. Hallucinated Dependencies

- **Non-existent packages** – AI invented the import
- **Wrong package names** – Similar but not real
- **Deprecated versions** – Old API that no longer exists

**Fix:** Verify package exists in registry before using.

### 8. Outdated Patterns

- **Deprecated APIs** – `componentWillMount`, old library methods
- **Legacy syntax** – `var`, `array()` in PHP, callbacks over async/await
- **Old framework patterns** – Superseded by better approaches

**Fix:** Use modern equivalents.

### 9. Missing Edge Cases

- **No null/undefined handling** – `array[0]` without length check
- **Empty input** – What happens with `""`?
- **Boundary conditions** – Off-by-one, max values

**Fix:** Add guards at system boundaries. Trust internal code.

### 10. YAGNI Violations

- **Unnecessary abstraction** – Factory for one implementation
- **Unnecessary files** – Could be 10 lines in existing file
- **Unnecessary methods** – One-liner that's called once
- **Config for one value** – Just hardcode it
- **"Future-proofing"** – Solving problems you don't have
- **Single-method classes** – Use a function instead
- **Wrapper classes** – Class that just calls another class
- **Interfaces with one implementation** – Abstraction without benefit

**Fix:** Delete it. Add when actually needed.

### 10a. Complexity Creep

Watch for these phrases that signal over-engineering:
- "Let's make it flexible for future requirements"
- "We should abstract this in case we need to change it"
- "Let's build a framework for this"
- "We need to make this configurable"
- "This needs to be extensible"
- "Let's create an interface for this"
- "We should decouple these components"
- "Let's implement the factory pattern here"

**Fix:** Say no. Solve the actual problem. Add complexity when proven needed.

### 11. Test Slop

- **Testing mocks** – Asserting mock was called, not real behavior
- **Incomplete mocks** – Missing fields the code depends on
- **No failure test** – Only happy path
- **Test-only methods** – Methods in production only called by tests
- **Over-mocking** – Mocking "to be safe" breaks real side effects
- **Tests as afterthought** – Code "complete" without tests

**Red flags:**
- Mock test IDs in assertions (`*-mock`)
- Methods only called in test files
- Mock setup >50% of test code
- Test fails when you remove mock

**Fix:** Test real behavior. TDD: failing test → implement → refactor.

### 12. Security Holes

- **Hardcoded secrets** – API keys, passwords in code
- **SQL injection** – String interpolation in queries
- **XSS** – Unescaped user input in output
- **Missing auth checks** – Endpoints without permission validation

**Fix:** Use env vars, parameterized queries, escape output, add auth.

### 13. Dead Code

- **Unused variables** – `_oldThing` for "compatibility"
- **Commented code** – `// old implementation`
- **Re-exports** – Keeping old API surface "just in case"
- **`// removed` markers** – Just delete it

**Fix:** Delete completely. Git has history.

## Red Flags

Stop if you see:
- Comments explaining obvious code
- Try/catch around simple operations
- `any`, `as`, `!` in TypeScript
- Same code block twice
- Empty catch or `catch(e) { log(e) }`
- Import you've never seen before
- `var`, `array()`, deprecated methods
- Files with one small function
- Methods called from exactly one place
- Class with only one method
- "Manager", "Service", "Helper" suffix on simple utilities
- Interface with single implementation
- Error logged but not handled or rethrown

## Process

1. **Get diff** – `git diff` or `git diff --cached`
2. **Scan each category** – Check against red flags
3. **Fix or flag** – Resolve issues or report blockers
4. **Verify clean** – Re-scan after fixes
5. **Only then** – Proceed with commit/PR

## Ecosystem References

- [bedrock.md](bedrock.md) – Laravel, WordPress, Acorn, Radicle patterns
- [typescript.md](typescript.md) – TypeScript-specific patterns

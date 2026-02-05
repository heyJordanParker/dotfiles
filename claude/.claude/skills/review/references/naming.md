# Naming

Gate for identifier quality. Names are the primary documentation.

**Core principle:** A name should tell you what it is, not make you read the code.

## The Gate

Before commit, scan changed identifiers for:

### 1. Misleading Names

- **Name doesn't match behavior** – `getUser()` that creates if missing
- **Semantic mismatch** – `isValid()` returns error message, not boolean
- **Stale names** – Function was refactored but name reflects old behavior
- **Verb mismatch** – `update` that deletes, `create` that upserts

**Fix:** Name must exactly describe what the code does.

### 2. Generic Names

- **Meaningless identifiers** – `data`, `info`, `result`, `item`, `handler`
- **Process verbs** – `processData()`, `handleEvent()`, `doWork()`
- **Manager/Service/Helper** – On simple utilities that don't manage anything

**Fix:** Name the specific thing. `userData` → `activeSubscribers`. `handleEvent` → `routeWebhook`.

### 3. Convention Violations

- **Naming style mismatch** – camelCase in snake_case file or vice versa
- **Inconsistent within file** – `userId` and `user_name` in same file
- **Import style mismatch** – Mixed named/default imports
- **Framework convention violation** – Not following library's naming patterns

**Fix:** Match the surrounding code exactly. Follow framework conventions.

### 4. Abbreviation Abuse

- **Unclear acronyms** – `usrAccMgr` instead of `userAccountManager`
- **Single-letter variables** – Outside of loop counters and lambdas
- **Domain-specific jargon** – Without context or documentation

**Fix:** Spell it out. Clarity > brevity.

### 5. Length Issues

- **Too short** – `p`, `fn`, `cb` for non-obvious things
- **Too long** – `getUserAccountByEmailAddressFromDatabase`
- **Boolean naming** – Should read as yes/no: `isActive`, `hasPermission`, `canEdit`

**Fix:** Short enough to scan, long enough to understand.

## Red Flags

- `get` prefix on function that mutates state
- `is`/`has` prefix on function returning non-boolean
- Same concept named differently across files
- Abbreviations that aren't universally understood
- Generic names in domain-specific code

## Process

1. **Get diff** – `git diff HEAD`
2. **List new identifiers** – Variables, functions, classes, types, files
3. **Check each against categories** – Misleading? Generic? Inconsistent?
4. **Suggest alternatives** – Concrete, specific, consistent names

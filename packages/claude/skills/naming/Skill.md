---
name: naming
description: MANDATORY for naming any code identifier — variable, function, file, folder, class, database column, route, CSS class. Always returns a 5-10 candidate slate; the consumer picks. TRIGGER when proposing or choosing a name, on /naming, on "rename", "ideas", "options", "what should I call", or any new file/class/method/column/route. DO NOT TRIGGER when editing a body that does not introduce or change an identifier, when restructuring code (architecture, not naming), or when labeling UI for end users (copy decision).
---

# Naming

**This skill is mandatory.** Follow these rules whenever naming anything in code.

## Output Contract

Every reply is a slate. Shape:

- **Slate** — 5-10 bulleted candidates. Each line: `` `candidate` — what it says; project precedent it matches; named failure mode if any concern remains ``
- **Recommended:** `name` — one-sentence reason
- **Runner-up:** `name` — one-sentence reason

When the caller co-tagged `/pcc`: each candidate becomes a `### name` section with a ` ```diff ` pros/cons block and a `Confidence: N%.` line. Recommended + Runner-up still follow.

**Banned openings** (each one triggers rejection):

- "Let's call it X"
- "I'd name it X"
- "The right name is X"
- "Recommended: X" without a slate above it
- Any single candidate before the slate

The slate is a thinking primer, not a multiple-choice ballot. The consumer often picks a name not in the slate after reading it.

## Quality Bar

Every candidate must pass three tests before entering the slate:

1. **Purpose** — name says what the caller gets, not how the thing works inside
2. **Domain language** — every word appears in the project's vocabulary or in plain conversational English a developer uses out loud
3. **One meaning** — name does not already mean something else in this codebase

Replace any candidate that fails any of the three before showing it.

## Hierarchy of Authority

1. **These rules** - non-negotiable baseline (e.g., no ALL_CAPS)
2. **Project conventions** - existing patterns in the codebase
3. **Language/framework conventions** - ecosystem standards

Always check the project first. Consistency within the project trumps external standards.

## Core Rules

- **Never ALL_CAPS for names** - use language features (`const`, `final`, `readonly`) to express immutability.

- **No abbreviations** - spell every word out.

- **Market-defined acronyms are fine** - `Url`, `Http`, `Api`, `Html`, `Css`, `Id` are acceptable. Don't invent project-specific acronyms users must learn.

- **Context informs naming** - the container (class, folder, namespace) provides context. `user.isValid()` not `user.isUserValid()`. `utils/dates.ts` not `date-utils.ts`.

- **No redundant suffixes** - `users` not `userList`. The type system or structure already tells you.

- **Hide implementation details** - name the interface, not the mechanism. `getUser` not `fetchAndCacheUser`.

- **Simple but complete** - don't over-shorten, but don't add words that don't add context.

- **No academic English** — thesaurus-substitute verbs from formal writing (`materialize`, `instantiate`, `synthesize`, `rehydrate`) make code read like a design doc; readers have to translate. CHECK: would a developer say this verb out loud at the keyboard? Examples: ✗ `materialize(data)` → ✓ `create(data)`; ✗ `instantiate(user)` → ✓ `createUser()`; ✗ `rehydrateSession()` → ✓ `loadSession()`.

- **No metaphor verbs** — verbs imported from unrelated fields (`mint` from currency, `prune` from gardening, `emit` from event systems used for rendering, `harvest` from farming) force the reader to translate. The metaphor only works when the field matches. CHECK: what field does the verb come from? If not the field the code is in, replace with the plain operational verb. Examples: ✗ `pruneRecords()` → ✓ `deleteRecords()`; ✗ `mintToken()` → ✓ `createToken()`; ✗ `emitNotification()` (for rendering) → ✓ `showNotification()`.

- **No vague verbs** — `process`, `handle`, `manage`, `do`, `run` convey nothing specific; reader has to open the body to learn what the function does. CHECK: can the verb be swapped with another generic verb without changing the name's meaning? If yes, name the actual operation. Examples: ✗ `processOrder()` → ✓ `shipOrder()` / `chargeOrder()` / `validateOrder()`; ✗ `manageSettings()` → ✓ `updateSettings()` / `loadSettings()`.

- **No overloaded terms** — when the name already means something specific in this codebase, reusing it forces every reader to disambiguate every time. CHECK: search the codebase for the proposed name; if it already names something distinct, add the qualifier that distinguishes the new thing. Examples: ✗ a new analytics record called `Event` when domain events already use `Event` → ✓ `TrackingEvent`; ✗ a payment provider class called `Provider` when service providers already use `Provider` → ✓ `PaymentGateway`.

- **No stutter** — type or file name repeats its module's word. The path reads longer, scans harder, and renaming the module forces touching every member. CHECK: drop the module's word from the type's name; if callers still read clearly, drop it. Examples: ✗ `users/UsersService` → ✓ `users/Service`; ✗ `auth/AuthMiddleware` → ✓ `auth/Middleware`; ✗ `models/UserModel` → ✓ `models/User`.

## Semantic Patterns

**Booleans**: Use `is`, `has`, `can`, `should` prefixes. `isLoggedIn`, `hasPermission`, `canEdit`.

**Event handlers vs callbacks**:
- Handler (internal): `handle` + event → `handleSubmit`
- Callback (prop): `on` + event → `onSubmit`

**Hooks**: `use` + what it provides → `useProducts`, `useAuth`

**Collections**: Simple plurals → `users`, `orders`. Not `userList`, `orderArray`.

**Transformers**: Method on the source object → `user.toJson()`, `order.toResponse()`

## The Naming Test (Boundary Detection)

Use when naming functions, services, or handlers. If you can't name it with one verb, the function is doing too many things.

For each function:
1. **Who is the caller?** Identify who uses this
2. **What is the step-level effect?** What does THIS function do — not the downstream chain, just its direct effect
3. **Name it with ONE idiomatic verb**

| Signal | Meaning |
|--------|---------|
| One verb covers all code paths | Boundary is correct |
| Need "or" to connect two verbs | Two operations bundled — split them |
| Name doesn't feel idiomatic | Boundary is wrong |
| Name matches a downstream effect, not this step | You're naming the chain, not the step |

**Step-level vs chain-level:** Name what THIS function does, not what its callees achieve. An orchestrator that calls validate → find → extract → insert is a `handler`, not an `adder`. The adding happens downstream.

**Caller perspective:** Names reflect what the caller achieves. A tool exposed externally: `placeLocale` (what the caller wants). The internal handler: `handlePlaceLocale` (what it does).

**Naming resistance as a signal:** If `resolveLocale` either pops from a list OR creates a new dict — "take" fits one path, "create" fits the other, need "or" → split into `extractLocale` and `createLocale`.

## Checklist

- [ ] Slate of 5-10 candidates, not a single name
- [ ] Each candidate passes the three Quality Bar tests (purpose, domain language, one meaning)
- [ ] Checked project conventions first
- [ ] No ALL_CAPS
- [ ] No abbreviations
- [ ] No academic English, metaphor verbs, or vague verbs
- [ ] No overloaded terms (name doesn't already mean something else here)
- [ ] No stutter (type name doesn't repeat module name)
- [ ] Context not repeated (user.isValid not user.isUserValid)
- [ ] No redundant suffixes (users not userList)
- [ ] Booleans use is/has/can/should prefix

## Slate Procedure

1. Read the surrounding code. Find sibling concepts already named. Identify the precedent shape
2. Generate 5-10 candidates varying the angle: action verb, thing noun, role, domain word, short vs descriptive
3. Scrub each candidate against every Core Rule. Replace dead candidates before they reach the slate
4. Annotate each line with what it says, the project precedent it matches, and the failure mode it tripped (if any)
5. Recommend one + name the runner-up with one-sentence reasons
6. Stop. The consumer picks.

## References

- [reference.md](reference.md) - Ecosystem casing conventions

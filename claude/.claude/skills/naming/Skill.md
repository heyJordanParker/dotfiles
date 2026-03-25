---
name: naming
description: MANDATORY for all naming decisions - variables, functions, files, folders, classes, database tables, routes, CSS classes. Must be followed when creating or renaming any identifier. Non-negotiable baseline for consistent, readable names across all languages and contexts.
---

# Naming

**This skill is mandatory.** Follow these rules whenever naming anything in code.

## Hierarchy of Authority

1. **These rules** - non-negotiable baseline (e.g., no ALL_CAPS)
2. **Project conventions** - existing patterns in the codebase
3. **Language/framework conventions** - ecosystem standards

Always check the project first. Consistency within the project trumps external standards.

## Core Rules

- **Never ALL_CAPS for names** - use language features (`const`, `final`, `readonly`) to express immutability. Exception: PHP `define()` constants follow WordPress convention. Note: Claude Code metadata files use capital case (Skill.md, Claude.md).

- **Avoid abbreviations** - spell words out. Exception: universally understood shortenings of long words (`info`, `max`, `min`, `config`).

- **Market-defined acronyms are fine** - `Url`, `Http`, `Api`, `Html`, `Css`, `Id` are acceptable. Don't invent project-specific acronyms users must learn.

- **Context informs naming** - the container (class, folder, namespace) provides context. `user.isValid()` not `user.isUserValid()`. `utils/dates.ts` not `date-utils.ts`.

- **No redundant suffixes** - `users` not `userList`. The type system or structure already tells you.

- **Hide implementation details** - name the interface, not the mechanism. `getUser` not `fetchAndCacheUser`.

- **Simple but complete** - don't over-shorten, but don't add words that don't add context.

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
| Need "or" to connect two verbs | Likely two operations bundled — split them |
| Name doesn't feel idiomatic | Boundary is wrong |
| Name matches a downstream effect, not this step | You're naming the chain, not the step |

**Step-level vs chain-level:** Name what THIS function does, not what its callees achieve. An orchestrator that calls validate → find → extract → insert is a `handler`, not an `adder`. The adding happens downstream.

**Caller perspective:** Names reflect what the caller achieves. A tool exposed externally: `placeLocale` (what the caller wants). The internal handler: `handlePlaceLocale` (what it does).

**Naming resistance as a signal:** If `resolveLocale` either pops from a list OR creates a new dict — "take" fits one path, "create" fits the other, need "or" → split into `extractLocale` and `createLocale`.

## Checklist

- [ ] Checked project conventions first
- [ ] No ALL_CAPS (except PHP define())
- [ ] No abbreviations (except ultra common & universal ones like info/max/min/config)
- [ ] Context not repeated (user.isValid not user.isUserValid)
- [ ] No redundant suffixes (users not userList)
- [ ] Booleans use is/has/can/should prefix

## References

- [reference.md](reference.md) - Ecosystem casing conventions
- [examples.md](examples.md) - Good/bad examples with rationale

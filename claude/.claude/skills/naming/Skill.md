---
name: naming
description: MANDATORY for all naming decisions - variables, functions, files, folders, classes, database tables, routes, CSS classes. Must be followed when creating or renaming any identifier. Non-negotiable baseline for consistent, readable names across all languages and contexts.
---

# Naming

**This skill is mandatory.** Follow these rules whenever naming anything in code.

## Hierarchy of Authority

1. **Project conventions** - existing patterns in the codebase
2. **Language/framework conventions** - ecosystem standards
3. **These principles** - fallback when no convention exists

Always check the project first. Consistency within the project trumps external standards.

## Core Rules

- **Never ALL_CAPS for names** - use language features (`const`, `final`, `readonly`) to express immutability. Exception: PHP `define()` constants follow WordPress convention.

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

## References

- [reference.md](reference.md) - Ecosystem casing conventions
- [examples.md](examples.md) - Good/bad examples with rationale

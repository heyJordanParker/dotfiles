---
name: naming
description: MANDATORY for naming any code identifier — variable, function, file, folder, class, database column, route, CSS class. Always returns a 5-10 candidate slate; the caller picks. TRIGGER when proposing or choosing a name, on /naming, on "rename", "ideas", "options", "what should I call", or any new file/class/method/column/route. DO NOT TRIGGER when editing a body that does not introduce or change an identifier, when restructuring code (Architecture, not naming), or when labeling UI for end Users.
---

# Naming

This Skill is mandatory whenever naming anything in code.

## 1. Read before naming

Read the surrounding code, find sibling concepts already named, and identify the Precedent shape before generating candidates.

## 2. Generate a slate

### Every reply is a slate
Return 5-10 candidates, then one recommendation and one runner-up. Never answer with a single name before the slate.

Template:
    - `candidate` — what it says; project Precedent it matches; named failure mode if any concern remains
    - `candidate` — what it says; project Precedent it matches; named failure mode if any concern remains

    Recommended: `name` — one-sentence reason.
    Runner-up: `name` — one-sentence reason.

Never: "Let's call it X", "I'd name it X", "The right name is X", "Recommended: X" without a slate above it, any single candidate before the slate.

### Every candidate contributes fresh words
No word appears twice across candidates. The slate's usefulness is the count of distinct fragments it puts in front of the caller. A candidate that reuses a word already on the slate gives nothing new. The only shared fragments allowed are the ones a Rule fixes: boolean `is`/`has`, Hook `use`, handler `handle`. The caller often recombines the fresh words into a final name that was not on the slate.

Example: naming a method that charges a failed payment on the backup card.

Never:
    retryWithBackupPaymentMethod / retryOnBackup / retryFailedCharge
    chargeBackupPaymentMethod / chargeFallbackMethod / chargeSecondaryMethod

Example:
    retryWithBackupPaymentMethod / chargeFallbackInstrument / recoverDeclinedSubscription
    reattemptOnSecondaryCard / billAlternateSource / captureDuesElsewhere
    salvageOverdueInvoice / collectViaSpareWallet

IF the caller co-tags `/pcc`:
### Keep the slate and change each candidate's shape
Each candidate becomes a `### name` section with a diff block for pros and cons and a `Confidence: N%.` line. The recommendation and runner-up still follow.

## 3. Vary the candidate angles

Generate candidates across action verb, thing noun, Domain word, short form, and descriptive form.

## 4. Scrub each candidate

Replace dead candidates before they reach the slate.

### Pass the three candidate tests
Every candidate says what the caller gets, not how it works inside; every word is in the project's vocabulary or in plain English a developer says out loud; the name does not already mean something else in this codebase.

### Follow the authority order
These Rules outrank project conventions, and project conventions outrank language or framework conventions. Check the project first; consistency within it trumps external standards.

### Never use `ALL_CAPS` for names
Express immutability with `const`, `final`, or `readonly`.

### Spell every word out
No abbreviations. Market acronyms are acceptable in code casing: `Url`, `Http`, `Api`, `Html`, `Css`, `Id`. Never invent project-specific acronyms.

### Let the container supply surrounding words
The class, folder, namespace, or module supplies surrounding words. Use `user.isValid()`, not `user.isUserValid()`; use `utils/dates.ts`, not `date-utils.ts`.

### Remove redundant suffixes
Use `users`, not `userList`; the type already says it.

### Hide implementation details
Name the interface, not the mechanism. Use `getUser`, not `fetchAndCacheUser`.

### Avoid academic English
Thesaurus verbs make code read like an Architecture doc. Use `create(data)`, not `materialize(data)`; use `loadSession()`, not `rehydrateSession()`.

### Avoid metaphor verbs
Verbs from unrelated fields force translation. Use `deleteRecords()`, not `pruneRecords()`; use `createToken()`, not `mintToken()`; use `showNotification()` when rendering, not `emitNotification()`.

### Avoid vague verbs
`process`, `handle`, `manage`, `do`, and `run` convey nothing specific. Name the actual operation: `shipOrder()`, `chargeOrder()`, `validateOrder()`, `updateSettings()`, `loadSettings()`.

### Avoid overloaded terms
When the name already means something specific in this codebase, add the qualifier that distinguishes the new thing. Use `TrackingEvent` for a new analytics record when domain events already use `Event`; use `PaymentGateway` for a payment provider when service providers already use `Provider`.

### Avoid stutter
Do not repeat the module's word in the type or file name. Use `users/Service`, not `users/UsersService`; use `models/User`, not `models/UserModel`.

### Follow semantic naming patterns
Booleans use `is`, `has`, `can`, or `should`: `isLoggedIn`, `hasPermission`, `canEdit`. Internal handlers use `handle` plus the event, like `handleSubmit`; prop callbacks use `on` plus the event, like `onSubmit`. Hooks use `use` plus what they provide, like `useProducts` or `useAuth`. Collections use simple plurals, like `users` or `orders`. Transformers live on the source object, like `user.toJson()` or `order.toResponse()`.

IF Project Precedent does not settle the casing convention:
### Follow ecosystem casing
TypeScript and JavaScript variables, functions, and constants use `camelCase`; classes, components, and types use `PascalCase`. PHP classes use `PascalCase`; methods and variables use `camelCase`; database tables, database columns, and route parameters use `snake_case`. Python uses `snake_case` except classes, which use `PascalCase`. Custom stylesheets use block-element-modifier naming; Tailwind and utility classes use `kebab-case`. React attributes use `camelCase`; vanilla HyperText Markup Language attributes use lowercase. Uniform Resource Locators and routes use `kebab-case` or `snake_case`, choosing the User-facing name. Database tables use plural `snake_case`; database columns and constraints use `snake_case`.

### Let the outer layer win when conventions conflict
A TypeScript response from a PHP backend keeps `snake_case` keys; do not transform names just to match JavaScript convention.

## 5. Run the split test

If a function, service, or handler cannot be named with one idiomatic verb, it is doing too many things.

### Name the direct step, not the downstream chain
Ask who the caller is, what direct effect this function has, and which one idiomatic verb names that effect. Needing `or` to connect two verbs means two operations are bundled. Split a `resolveLocale` that pops from a list or creates a new object into `extractLocale` and `createLocale`.

### Use the caller's perspective
Name the external tool for what the caller wants, and name the internal handler for what it does. Use `placeLocale` for the tool and `handlePlaceLocale` for the handler.

## 6. Recommend one and stop

Name one recommendation and one runner-up, then stop. The caller picks.

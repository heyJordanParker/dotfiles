---
name: naming
description: MANDATORY for naming any code identifier — variable, function, file, folder, class, database column, route, CSS class. Always returns a 5-10 candidate slate; the caller picks. TRIGGER when proposing or choosing a name, on /naming, on "rename", "ideas", "options", "what should I call", or any new file/class/method/column/route. DO NOT TRIGGER when editing a body that does not introduce or change an identifier, when restructuring code (Architecture, not naming), or when labeling UI for end Users.
---

# Naming

This Skill is mandatory whenever naming anything in code.

## 1. Find the Precedent

### Copy the shape of the names around it
Read the folder, file, class, or table the name lands in. The names already there set the shape: singular or plural, noun or verb, one word or two, prefixed or bare. Every candidate uses that shape.

### Follow the project's domain language
Use the words this project already speaks. A word it spends on something else is taken. A word it retired stays retired. Read the project's `Domain.md` if it exists.

## 2. Pick the angles

### Give every candidate its own angle
An angle is a different idea of what the thing is, not a different wording of one idea. Frame the thing several ways first: what the caller gets, the Domain event, the object that changes, the state after, the trigger before, the real-world act. Each candidate comes off a different framing and names it on its slate line. Two candidates share an angle when one sentence describes both, so replace one. The reply opens on the first candidate, never on a list of angles. The candidates below name a method that charges a failed payment on the backup card.

Never:
    chargeFallbackInstrument / billAlternateSource / captureDuesElsewhere
    collectViaSpareWallet / reattemptOnSecondaryCard

Example:
    retryCharge — the same operation runs again
    recoverSubscription — the subscription returns to active
    useBackupCard — the stored second card is what changes
    settleBalance — the debt reaches zero
    rescueAccount — the customer keeps their access

## 3. Generate a slate

### Every reply is a slate
Return 5-10 candidates, then one recommendation and one runner-up. Never answer with a single name before the slate.

Template:
    - `candidate` — the angle in one clause; project Precedent it matches; the cost it carries if it has one
    - `candidate` — the angle in one clause; project Precedent it matches; the cost it carries if it has one

    Recommended: `name` — one-sentence reason.
    Runner-up: `name` — one-sentence reason.

Never: "Let's call it X", "I'd name it X", "The right name is X", "Recommended: X" without a slate above it, any single candidate before the slate.

### Change every fragment in every candidate
A word that appears in one candidate appears in no other. The caller mixes words across candidates into a final name that was not on the slate, so a repeated word costs a slot and returns nothing.

Never:
    deleteFiles / deleteManyFiles / deleteData

IF no alternative to a word survives the scrub:
### Fix that word and spend the slate on the rest
Say in one clause that the word is settled, then change every other word across the slate. Settling is rare. A word stays open unless you can name why each alternative fails. The `use` that opens a Hook name is one of the few words already settled elsewhere in this Skill.

IF the caller co-tags `/pcc`:
### Keep the slate and change each candidate's shape
Each candidate becomes a `### name` section with a diff block for pros and cons and a `Confidence: N%.` line. The recommendation and runner-up still follow.

## 4. Scrub each candidate

Replace dead candidates before they reach the slate.

### Cut a candidate that fails a test, keep one that only costs something
A word the project already uses for something else, or a word outside the project's language, fails. Cut it whatever its angle is worth. Length, a heavy compound, and a near miss with a sibling are costs. Those candidates stay, with the cost named.

### Redraw a dead candidate on its own angle
A candidate that fails a test leaves its angle open. Find another word for that angle. Drop the angle only when no word survives on it.

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

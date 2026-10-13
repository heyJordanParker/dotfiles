---
name: naming
description: Name a code identifier, or review the names code already has — variable, function, class, file, folder, database column, route, CSS class, enum case, attribute. TRIGGER when introducing or changing an identifier, on /naming, "rename", "what should I call", "naming ideas", or "review the naming". DO NOT TRIGGER when editing a body that introduces no identifier, when restructuring code (that is Architecture), or when writing UI copy for end Users.
---

# Naming

The files, the Public API, the database, and the names in them form the application's language, and the Architect reviews them as Architecture. A name is right when its full callsite reads as a plain sentence, in the project's domain language, about the Capability it gives the caller. The Architect builds the final name from words across the candidates, so each word on the list is a separate option.

## 1. Follow the project's conventions and language

Read the names next to the new one, its current callers, and the project's `Domain.md`. Follow the conventions those names use over your own defaults, and the framework's and the language's conventions where the project has none. Use the words the domain and the Architect already use for this Capability, never a word the project stopped using.
Example: Cashier's `Subscription` has `onTrial()` and `onGracePeriod()`, so its new state check is `onBillingPause()`, not `isPaused()`.
Example: a TypeScript type for a PHP response keeps the PHP side's `snake_case` keys.

## 2. Give each candidate a different angle

An angle is a different idea of what the thing is: what the caller gets, the domain event, the object that changes, the state after, the trigger before, or the real-world act. Two candidates have the same angle when one sentence describes both. These candidates name a method on `Payment` that charges a failed payment to the backup card.
Never: `$payment->chargeFallbackInstrument()` / `$payment->billAlternateSource()`
Example:
    $payment->retryCharge() — the same operation runs again
    $payment->recoverSubscription() — the subscription returns to active
    $payment->useBackupCard() — the stored second card is what changes
    $payment->settleBalance() — the debt reaches zero

### Use each word in one candidate only
When every candidate needs the same word, such as the noun each sibling names, say once above the list that the word is fixed, and vary the other words.
Never: `checkStatus` / `checkInfo` — three words for two candidates.

## 3. Test each candidate as a sentence at its callsite

Write each candidate into a full callsite: a current caller, or the call the change adds. A static member includes its class, and an instance member includes the project's usual variable name. For a name on a Module's Public API, use a callsite in another Module. Read the line as one plain sentence and put each word the reader has to add in parentheses. When every reader adds the same word, leave it out of the name. When readers would add different words, put the word in the name or remove the candidate.
Example: `$order->total()` reads "the order's total (before or after tax)". Readers add different words, so the name has to say which.

Remove a candidate whose sentence:
- says how the code works instead of what the caller gets: `fetchAndCacheUser` for `getUser`
- repeats what its container or type already says: `$user->isUserValid()`, `userList`
- has a word the reader must translate: an abbreviation (`cfg`), an academic or metaphor word (`materialize`, `mint`, `prune`), or an empty verb (`process`, `handle`, `manage`)
- uses a word that means something else in the project or its framework, unless a qualifier tells the two apart: `TrackingEvent` where domain events are `Event`. Search the code for each word.
- needs "and" or "or": the code does two jobs, so report the split instead of a name

Replace a removed candidate with another word for the same angle. Keep a candidate that is only long or close to a sibling, and state that downside.

## 4. Reply with the candidates

List 5 to 10 candidates, each in its callsite, then a recommendation and a runner-up, and stop. A review of existing names lists every name whose sentence fails, each with its sentence and its own candidates.
Never: a name chosen before the list.

Template:
    - `$caller->candidate()` — its angle; its sentence; its downside, if it has one
    Recommended: `candidate` — one-sentence reason.
    Runner-up: `candidate` — one-sentence reason.

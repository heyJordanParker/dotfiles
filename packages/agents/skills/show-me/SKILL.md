---
name: show-me
description: Draw Public APIs, Module shape, Prompt changes, Architecture, Capabilities, and UX as diffed trees and terminal diagrams, so the Architect understands a reply by scanning it. TRIGGER on every reply that explains code, Architecture, a change, a name, or a finding, and whenever the Architect says "show me". DO NOT TRIGGER to score a set of options; that is /rank.
---

# Show Me

- The Architect reviews Capabilities and Architecture, never code, and scans a reply instead of reading it word for word.
- This Skill owns every drawing in a reply. communication.md owns its sentences, and /present owns its structure when it holds a Proposal or a report.
- Every drawing lives in the reply, in a code block the terminal renders. This Skill writes no files.

## 1. Draw each idea under the sentence that states it

Pick the smallest drawing that proves the idea. The sentence above it states the idea in terms of Users, Capabilities, or maintenance. Draw the full shape around the change, because the unchanged parts are what the Architect judges the change against.

### Diff every change
Put `+` or `-` as the first character of each added or removed row, in trees, step lists, and code alike. Mark an item edited in place with a `*` suffix.

### Keep drawings vertical and under 100 columns
The terminal wraps long lines and breaks things placed side by side. Flow every drawing top to bottom, one item per row, with room after each row for a `<-` note.
Never: Mermaid, which the terminal shows as source, two boxes on one row, or labels in a column beside their values.

### Draw data that is a table as one table
Numbers the Architect compares across rows, such as counts per repository, are one table in a `text` block. Keep every row on one line under 100 columns, and draw everything else as a tree or prose.
Never: a table for information that is not rows of numbers, or one table after another.

## 2. Draw Public APIs and Module shape as an annotated file tree

One tree serves one file or many. List every public method of each file the change reaches, changed or not, plus the other methods the change relies on. A note after `<-`, under nine words, says why the row exists or changed. On the line after the block, `Key Files:` links each file that still exists.
Example:
  Only a team's owner can delete it now, and one `sync()` replaces two grant methods.
  ```diff
    app/
    ├── Teams/TeamService.php*
  - │   ├── delete(Team $team): void
  + │   ├── #[OwnerOnly] delete(Team $team): void  <- admins deleted teams by mistake
    │   └── create(string $name, User $owner): Team
    └── Billing/
        ├── AccessService.php*                   <- access follows the paid order
  +     │   ├── sync(Order $order): void         <- one path for every purchase
  -     │   ├── grantOrder(int $orderId): void
  -     │   └── grantItem(OrderItem $item): void
  +     └── PriceSnapshot.php                    <- the price at checkout
  +         └── of(Order $order): PriceSnapshot
  ```
  Key Files: [TeamService.php](/srv/shop/app/Teams/TeamService.php), [AccessService.php](/srv/shop/app/Billing/AccessService.php), [PriceSnapshot.php](/srv/shop/app/Billing/PriceSnapshot.php)

The same tree shows a database table with every column, and a name under review among its neighbours.

## 3. Show code and Prompt changes as diffs

Above each diff, write `In`, a link to the file, what the diff cannot show, and a colon. Quote the diff in a blockquote, one file per diff. Summarize the code around the change in comments, and mark skipped lines with `⋯`.
Example:
  In [AccessService.php](/srv/shop/app/Billing/AccessService.php), the check before any grant:
  > ```diff
  >   public function sync(Order $order): void
  >   {
  > -     if ($order->synced_at !== null) {
  > +     if ($this->grants->existFor($buyer, $order)) {
  >           return;
  >       }
  >       // grants every product, then emails the buyer
  >   }
  > ```

IF a Prompt change rewrites more than one section:
### Show the Capabilities and Principles it changes, then its headings
Draw what Agents can now do as a Capabilities tree, then diff the Prompt's headings as a tree. Show a smaller Prompt change as its full diff.

## 4. Draw Architecture, flows, and logic as vertical diagrams

Stack boxes from the entry point down to the final effect. A box lists, one per row, only the methods or fields the idea needs. Every arrow names its behavior, such as `calls`, `reads`, `writes`, or `emits`. A reshaped Architecture is two diagrams, before above after.
Example:
  Every access check asks a Group, so a Team and each later kind of group share one check.
  ```
  ┌──────────────────┐
  │ AccessService    │
  │ ├── check()      │
  │ └── sync()       │
  └────────┬─────────┘
           │ calls includes()
           ▼
  ┌──────────────────┐
  │ Group            │
  └────────▲─────────┘
           │ implements
  ┌────────┴─────────┐
  │ Team             │
  └──────────────────┘
  ```

Draw a paragraph's logic as a call tree, pseudocode, or the states and the events between them.
Example:
  ```
  running
     ├── answer written ──> ok
     ├── non-zero exit ───> failed
     └── cancel ──────────> cancelled
  ```

## 5. Map the Capabilities, the Module, and the UX a change touches

Agents leave these out, and the Architect designs with them. Each is a tree, so any row can carry a `<-` note.

### Draw the Capabilities with their WHY when a change adds, removes, or changes one
Template:
  ```diff
    Checkout                         <- the Module that owns them
  + ├── retry a failed payment       <- 4% of buyers leave after a decline
    └── pay with a saved card
  ```

### Draw the Module when a change adds or reshapes one
Template:
  ```diff
    Billing                            <- every payment a buyer makes
    ├── Capabilities
    │   ├── charge a buyer
    │   ├── refund a payment
  + │   └── retry a failed payment     <- 4% of buyers leave after a decline
    ├── Public API
    │   ├── charge(Order): Payment
  + │   ├── retry(Payment): Payment
    │   └── refund(Payment): void      <- refunds the whole payment only
    └── Depends on
        ├── Orders                     <- reads the order total
        └── Stripe
  ```

### Draw the UX as numbered steps when what the User does changes
Template:
  ```diff
    1. The buyer presses Pay
  + 2. The buyer sees "Card declined" and a Retry button
  + 3. The buyer picks another card and presses Retry
    4. The buyer lands on the receipt
  ```

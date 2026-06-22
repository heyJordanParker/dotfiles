---
name: show-architecture
description: Use when exploring/explaining code, designing/updating architecture, or when asked to visualize structure. Shows annotated file trees inline.
---

# Show Architecture

Use annotated file trees to visualize and explain architecture.

## When to Use

- Exploring unfamiliar code
- Explaining how a feature works
- Designing new architecture
- Reviewing or updating structure
- On explicit request

## Format

```
directory/
├── file.ts*             <- annotation (3-5 words)
├── subdirectory/
│   ├── nested.ts*       <- changed file marked with *
│   └── related.ts       <- context file (no *)
└── context.ts
```

## Rules

1. **Box-drawing:** `├──`, `└──`, `│` for structure
2. **Annotations:** `<-` arrow, brief (3-5 words)
3. **Changed files:** mark with `*` suffix (like commit)
4. **Context-dependent:** adapt annotations to purpose
5. **Skip irrelevant:** only show relevant files, omit the rest entirely
6. **Never write to files.** Output inline only. No exceptions.
7. **No status prefixes.** Mark a changed file with `*` and state the change after `<-`. Show an unchanged context file with a plain role annotation and no prefix. Never write `KEEP:`, `REMOVE:`, or `PRESERVE:`.

## Annotation Styles

**Overview** (responsibilities):
```
src/
├── core/
│   ├── engine.ts*       <- orchestrates subsystems
│   └── config.ts        <- runtime settings
├── adapters/
│   ├── http.ts*         <- express server
│   └── db.ts            <- postgres connection
└── index.ts             <- entrypoint
```

**Feature deep-dive** (data flow):
```
src/auth/
├── login.ts             <- receives credentials
├── validate.ts*         <- checks against db
├── token.ts*            <- issues JWT
└── middleware.ts        <- verifies on requests
```

**Debugging** (dependencies):
```
src/
├── api/handler.ts       <- calls UserService
├── services/
│   └── UserService.ts   <- calls Repository
└── repos/
    └── UserRepo.ts*     <- fails here
```

## Anti-patterns

- Showing every file (overwhelming)
- Missing annotations (useless tree)
- Annotations that repeat filename
- `* new` annotations — the `*` already conveys it; the annotation describes the role
- `existing,` prefix on context-file annotations — describe the role directly, mention the relationship to the change only when load-bearing (e.g., `Untracked.php   <- separate purpose; we add NoAudit instead`)

## Relationship Diagrams

File trees show structure. A relationship diagram shows runtime flow: how components call, read, write, or hand off. Use it when the question is "how does data move through this", not "where do the files live".

```
┌─────────────────────────────────┐
│ Cart  (source of truth)         │
│ items: offer_id, product_id, qty│
│ subtotal / tax / total          │
└──────┬───────────────────┬──────┘
       │ read              │ read
       ▼                   ▼
┌──────────────┐   ┌──────────────────┐
│ CheckoutView │   │ StoreService     │
│ customer sees│   │ projectCartToWc()│
└──────────────┘   └────────┬─────────┘
                            │ write
                            ▼
                   ┌──────────────────┐
                   │ WC_Order         │
                   └────────┬─────────┘
                            │ charge
                            ▼
                   ┌──────────────────┐
                   │ Gateway plugin   │
                   └──────────────────┘
```

### Rules

1. The box title is the component. Inside, list only the fields or methods the flow touches.
2. Every arrow is labelled with the relationship: `read`, `write`, `charge`, `emit`, `call`.
3. Mark the authoritative box, for example `(source of truth)`.
4. Flow runs top to bottom: entry at the top, terminal effect at the bottom.
5. Show only the boxes on the path being explained. Skip the rest.

### Tree or diagram

- **File tree.** Where code lives, what files change.
- **Relationship diagram.** How it works end to end, data flow, ownership, call order.

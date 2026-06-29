# Decisions Format

Decisions live in `docs/architecture/decisions/` and are numbered: `0001-{decision-name}.md`, `0002-{decision-name}.md`. The filename is the actual decision, not a slug of a category.

## Template

```md
# {The decision}

{1-3 sentences: what the situation was, what was decided, and why.}
```

An entry can be a single paragraph. The value is recording *that* a decision was made and *why* — not filling out sections.

## When to record a decision

All three must be true:

1. **Hard to reverse** — the cost of changing your mind later is meaningful
2. **Not self-evident from the code** — a future reader will look at the code and wonder "why on earth did they do it this way?"
3. **The result of a real trade-off** — there were genuine alternatives and you picked one for specific reasons

If a decision is easy to reverse, skip it — you'll just reverse it. If it's obvious, nobody will wonder why. If there was no real alternative, there's nothing to record.

## What qualifies

- **Architectural shape.** "The write model is event-sourced, the read model is projected into Postgres."
- **Integration patterns between systems.** "Ordering and Billing communicate via domain events, not synchronous HTTP."
- **Technology choices that carry lock-in.** Database, message bus, auth provider, deployment target — the ones that would take a quarter to swap out.
- **Boundary and scope decisions.** "Customer data is owned by the Customer system; others reference it by id only." The explicit no's are as valuable as the yes's.
- **Deliberate deviations from the obvious path.** "Manual SQL instead of an ORM because X." Anything where a reasonable reader would assume the opposite — these stop the next engineer from "fixing" something deliberate.
- **Constraints not visible in the code.** "Response times must be under 200ms because of the partner API contract."

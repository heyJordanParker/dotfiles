---
name: laravel
description: "Apply when writing, reviewing, or refactoring Laravel PHP code — controllers, models, migrations, queries, jobs, caching, validation, security, and architectural patterns. Also triggers for Inertia, Livewire, Breeze, and Jetstream work."
user-invocable: false
license: MIT
metadata:
  author: heyJordanParker
  upstream: laravel/boost
---

# Laravel Best Practices

## Consistency First

Before applying any practice, check what the application already does. The best choice is the one the codebase already uses, even if another pattern would be theoretically better. Inconsistency is worse than a suboptimal pattern.

Check sibling files, related controllers, models, or tests for established patterns. If one exists, follow it — don't introduce a second way. These are defaults for when no pattern exists yet, not overrides.

## Universal Practices

These apply to nearly every Laravel task. The reference files below provide code examples and edge cases for specific workflows.

### Performance

- Eager load with `with()` — never lazy load. Enable `Model::preventLazyLoading()` in dev
- `withCount()` instead of loading relations just to count
- `chunk()` / `chunkById()` for large datasets — never `Model::all()` in loops
- Select only needed columns — include foreign keys in eager load selects
- Index columns in `WHERE`, `ORDER BY`, `JOIN`
- No queries in Blade templates — pass data from controllers

### Security

- Define `$fillable` on every model — never `$guarded = []` on models accepting user input
- Authorize every action via policies or gates
- Never interpolate user input into queries — use Eloquent or parameter binding
- `{{ }}` for output, `{!! !!}` only for trusted pre-sanitized content
- `@csrf` on all POST/PUT/DELETE forms
- `encrypted` cast + `$hidden` for sensitive database fields

### Architecture

- Constructor dependency injection — never `app()` or `resolve()` inside classes
- Controllers under 10 lines per method — extract to Action or service classes
- Form Request classes for validation — never inline in controllers
- `$request->validated()` only — never `$request->all()`
- Implicit route model binding — `show(Post $post)` not `show(int $id)`
- Default `ORDER BY id DESC` or `created_at DESC` — explicit ordering always
- `env()` only in config files — direct calls return `null` when config is cached
- Follow conventions — don't override table names, primary keys, or pivot naming unless necessary

### Eloquent

- Return type hints on relationships (`HasMany`, `BelongsTo`, etc.)
- Attribute casts in `casts()` method — cast all date columns, booleans, JSON
- Local scopes for reusable query constraints — global scopes only for universal filters (soft deletes, tenancy)

### Async

- `retry_after` must exceed job `timeout` — otherwise jobs duplicate while still running
- Always implement `failed()` on jobs
- `ShouldDispatchAfterCommit` on events/notifications inside transactions
- `afterCommit()` on queued mailables inside transactions

## References

Read the relevant reference based on what you're doing:

- [optimizing-queries.md](references/optimizing-queries.md) — "I need efficient queries" — N+1 prevention, subqueries, chunking, indexes, collections
- [writing-models.md](references/writing-models.md) — "I'm setting up a model" — relationships, scopes, casts, `whereBelongsTo`
- [securing-code.md](references/securing-code.md) — "I need to make this secure" — mass assignment, authorization, SQL injection, XSS, CSRF, file uploads, encryption
- [validating-input.md](references/validating-input.md) — "I need to validate user input" — Form Requests, `validated()`, conditional rules
- [structuring-code.md](references/structuring-code.md) — "I'm organizing code or setting up routes" — Action classes, DI, route model binding, resource routes, `defer()`, `Context`, config
- [writing-migrations.md](references/writing-migrations.md) — "I need to create a migration" — foreign keys, indexes, defaults, immutability, DDL/DML separation
- [writing-tests.md](references/writing-tests.md) — "I need to write tests" — database refresh, fakes ordering, factories, assertions
- [handling-async.md](references/handling-async.md) — "I need background work" — jobs, retry, backoff, events, notifications, mail, scheduling
- [calling-apis.md](references/calling-apis.md) — "I need to call an external API" — timeouts, retry, pooling, error handling, faking
- [handling-errors.md](references/handling-errors.md) — "I need error handling" — exception reporting, rendering, throttling, JSON APIs
- [implementing-caching.md](references/implementing-caching.md) — "I need caching" — `remember()`, `flexible()`, `once()`, locks, tags
- [building-views.md](references/building-views.md) — "I'm working on templates" — Blade components, attributes, composers, naming conventions, helpers

---
paths: "**/*.php"
---

# Laravel

When working on PHP files in a Laravel project, proactively load the `/laravel` skill. Read the reference that matches the current task:

- Queries, performance → `references/optimizing-queries.md`
- Models, relationships → `references/writing-models.md`
- Controllers, routes, code organization → `references/structuring-code.md`
- Validation, forms → `references/validating-input.md`
- Security, authorization → `references/securing-code.md`
- Migrations → `references/writing-migrations.md`
- Tests → `references/writing-tests.md`
- Jobs, events, scheduling → `references/handling-async.md`
- External APIs → `references/calling-apis.md`
- Error handling → `references/handling-errors.md`
- Caching → `references/implementing-caching.md`
- Blade, frontend → `references/building-views.md`

## Principles

- Consistency over correctness — match existing project patterns before applying any rule
- Constructor injection everywhere — never `app()` or `resolve()`
- Form Requests own validation — never inline in controllers
- `$request->validated()` only — never `$request->all()`
- Authorize every action — policies or gates, no exceptions
- `env()` only in config files — never in application code
- Eager load relationships — never lazy load
- Explicit ordering — always `ORDER BY`, never rely on database defaults

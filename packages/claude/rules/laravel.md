---
paths: "**/*.php"
---

IF working on PHP files in a Laravel project:
### Load /laravel proactively
Load the /laravel Skill.

IF working on Laravel queries or performance:
### Read `references/optimizing-queries.md`
Read `references/optimizing-queries.md`.

IF working on Laravel models or relationships:
### Read `references/writing-models.md`
Read `references/writing-models.md`.

IF working on Laravel controllers, routes, or code organization:
### Read `references/structuring-code.md`
Read `references/structuring-code.md`.

IF working on Laravel validation or forms:
### Read `references/validating-input.md`
Read `references/validating-input.md`.

IF working on Laravel security or authorization:
### Read `references/securing-code.md`
Read `references/securing-code.md`.

IF working on Laravel migrations:
### Read `references/writing-migrations.md`
Read `references/writing-migrations.md`.

IF working on Laravel tests:
### Read `references/writing-tests.md`
Read `references/writing-tests.md`.

IF working on Laravel jobs, events, or scheduling:
### Read `references/handling-async.md`
Read `references/handling-async.md`.

IF working on Laravel external APIs:
### Read `references/calling-apis.md`
Read `references/calling-apis.md`.

IF working on Laravel error handling:
### Read `references/handling-errors.md`
Read `references/handling-errors.md`.

IF working on Laravel caching:
### Read `references/implementing-caching.md`
Read `references/implementing-caching.md`.

IF working on Laravel Blade or frontend:
### Read `references/building-views.md`
Read `references/building-views.md`.

### Match project patterns first
Consistency outranks correctness. Match existing project patterns before applying any Rule.

### Use constructor injection everywhere
Use constructor injection everywhere.
Never: `app()` or `resolve()`.

### Put validation in Form Requests
Form Requests own validation.
Never: inline validation in controllers.

### Use `$request->validated()` only
Use `$request->validated()` for request data.
Never: `$request->all()`.

### Authorize every action
Authorize every action with policies or gates. No exceptions.

### Use `env()` only in config files
Use `env()` only in config files.
Never: `env()` in application code.

### Eager load relationships
Eager load relationships.
Never: lazy load relationships.

### Order queries explicitly
Always add `ORDER BY`.
Never: rely on database defaults.

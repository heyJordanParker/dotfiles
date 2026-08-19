---
paths: "**/*.php"
---

IF the project is Laravel:
### Use /laravel proactively
Use /laravel.

### Match project patterns first
Consistency outranks correctness. Match existing project patterns before applying any Rule.

### Use constructor injection everywhere
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
Never: `env()` in application code.

### Eager load relationships
Never: lazy load relationships.

### Order queries explicitly
Always add `ORDER BY`.
Never: rely on database defaults.

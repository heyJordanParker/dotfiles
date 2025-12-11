---
name: architecture-review
description: Use when reviewing code changes for architectural issues - SOLID violations, encapsulation breaks, wrong dependency direction, bi-directional dependencies, improper separation of concerns. Complements anti-slop (which catches YAGNI/over-engineering).
---

# Architecture Review

Gate for catching structural issues before they compound into maintenance nightmares.

**Core principle:** Dependencies flow one direction. Modules hide their internals.

## The Rule

Dependencies point inward: High-level → Low-level, App → Libraries, Controllers → Services → Models.

## Categories

### 1. SOLID Violations

**Single Responsibility**
- Class doing multiple unrelated things
- File mixing concerns (UI + business logic + data)
- Method with multiple reasons to change

**Open/Closed**
- Modifying existing code for every new case (extend instead)
- Switch statements that grow with each feature

**Liskov Substitution**
- Subclass breaks parent's contract
- Override that changes expected behavior

**Interface Segregation**
- Interface with methods clients don't need
- Forcing implementations of unused methods

**Dependency Inversion**
- High-level module importing low-level details
- Business logic depending on infrastructure

### 2. Encapsulation Breaks

- **Exposed internals** - Public fields that need to be private
- **Leaky abstractions** - Implementation details in public API
- **Direct property access** - Bypassing getters/setters with logic
- **Friend functions** - External code knowing internal structure

**Fix:** Hide internals. Expose behavior, not data.

### 3. Dependency Direction

- **Bi-directional dependencies** - A imports B, B imports A
- **Circular references** - A → B → C → A
- **Upward dependencies** - Utility depending on business logic
- **Cross-module coupling** - Sibling modules importing each other

**Fix:** Dependencies point one way. Extract shared code to lower layer.

### 4. Separation of Concerns

- **Mixed layers** - Database queries in UI components
- **Business logic in controllers** - Move to services/models
- **Infrastructure in domain** - Domain knowing about HTTP/DB
- **Presentation in data layer** - Formatting in models

**Fix:** Each layer has one job. Pass data between layers.

## Red Flags

Stop if you see:
- Module A imports from Module B AND Module B imports from Module A
- Business logic in a controller or route handler
- Database/HTTP calls in React components
- Class with 5+ unrelated methods
- Method with 3+ responsibilities
- Public fields without encapsulation reason
- Inheritance used for code reuse (use composition)

## Process

1. **Map dependencies** - Which modules import which?
2. **Check direction** - All arrows point same way?
3. **Find cycles** - Any circular references?
4. **Verify layers** - Each layer doing its job only?
5. **Check encapsulation** - Internals hidden?

## Quick Reference

| Issue | Fix |
|-------|-----|
| Bi-directional dependency | Extract shared code to third module |
| Business logic in controller | Move to service/model |
| Exposed internal state | Make private, add behavior methods |
| God class | Split by responsibility |
| Mixed concerns in file | Separate into focused files |
| Circular import | Invert dependency or extract interface |

## Not Covered Here

These are in `anti-slop`:
- YAGNI violations
- Premature abstraction
- Over-engineering
- Unnecessary complexity

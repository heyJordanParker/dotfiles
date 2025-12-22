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
3. **Changed files:** mark with `*` suffix (like commit-messages)
4. **Context-dependent:** adapt annotations to purpose
5. **Skip irrelevant:** only show relevant files, omit the rest entirely
6. **Never write to files.** Output inline only. No exceptions.

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

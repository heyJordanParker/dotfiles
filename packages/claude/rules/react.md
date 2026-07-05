---
paths: "**/*.tsx, **/*.jsx"
---

IF working on React files:
### Load /react proactively
Load the /react Skill.

IF building React components:
### Read `references/composition.md`
Read `references/composition.md`.

IF managing React state or writing hooks:
### Read `references/state-management.md`
Read `references/state-management.md`.

IF deciding whether to use a React effect:
### Read `references/effects.md`
Read `references/effects.md`.

IF optimizing React performance:
### Read `references/performance.md`
Read `references/performance.md`.

IF fetching data in React:
### Read `references/data-fetching.md`
Read `references/data-fetching.md`.

IF building React forms:
### Read `references/forms.md`
Read `references/forms.md`.

IF working with React Server Components or Actions:
### Read `references/server-rendering.md`
Read `references/server-rendering.md`.

IF writing React tests:
### Read `references/testing.md`
Read `references/testing.md`.

IF setting up React routes:
### Read `references/routing.md`
Read `references/routing.md`.

IF styling React:
### Read `references/styling.md`
Read `references/styling.md`.

IF handling React errors:
### Read `references/error-handling.md`
Read `references/error-handling.md`.

IF reviewing React security:
### Read `references/security.md`
Read `references/security.md`.

IF working on React accessibility:
### Read `references/accessibility.md`
Read `references/accessibility.md`.

IF working on React project structure:
### Read `references/project-structure.md`
Read `references/project-structure.md`.

IF working on TypeScript types in React:
### Read `references/typescript.md`
Read `references/typescript.md`.

### Check sibling files first
Check sibling files for existing patterns first. Consistency beats any Rule here.

### Use effects only for external system sync
Effects are escape hatches for external system sync. Never use effects for derived state or user events. Derive state during render. Event-specific logic goes in event handlers.

### Compose instead of configuring
Use compound components with shared context, explicit variants, and children over render props.
Never: boolean prop proliferation.

### Keep state close to usage
Keep state as close to usage as possible. Use TanStack Query/SWR for asynchronous data, useState/Zustand for client state, nuqs for URL state, and Context for read-heavy/write-rare only.

### Never define components inside components
Never define components inside components. It causes full remount every render.

### Import directly instead of from barrel files
Never import from barrel files. Direct imports save 200-800ms.

### Put `ErrorBoundary` outside `Suspense`
`ErrorBoundary` wraps `Suspense`, not inside it. Use granular per-section boundaries.

### Test behavior, not implementation
Test behavior, not implementation. Query priority is `getByRole` > `getByLabelText` > `getByText` > `getByTestId`.

### Use semantic HTML first
Use semantic HTML first.
Example: `<button>` instead of `<div role="button" tabIndex={0} onClick>`.

### Avoid inline styling
Do not use inline styles, style objects, or conditional className string-building.

### Treat React escaping as narrow
React auto-escapes `{value}` but not `dangerouslySetInnerHTML`, URLs, or `ref.innerHTML`.

IF using React 19:
### Prefer `use()` over `useContext()`
`use()` replaces `useContext()` and can be called conditionally.

IF using React 19:
### Treat `ref` as a regular prop
`ref` is a regular prop. `forwardRef` is deprecated.

IF using React 19:
### Use `useEffectEvent` for event logic in effects
`useEffectEvent` separates event logic from effect dependencies.

IF using React 19:
### Let React Compiler memoize
React Compiler auto-memoizes. Manual `memo`, `useMemo`, and `useCallback` are often unnecessary.

IF using React 19:
### Use React 19 form hooks
Use `useActionState` for form state, `useFormStatus` for pending, and `useOptimistic` for immediate feedback.

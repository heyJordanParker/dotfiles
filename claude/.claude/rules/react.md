---
paths: "**/*.tsx, **/*.jsx"
---

# React

When working on React files, proactively load the `/react` skill. Read the reference that matches the current task:

- Building components → `references/composition.md`
- Managing state or writing hooks → `references/state-management.md`
- Deciding whether to use an effect → `references/effects.md`
- Optimizing performance → `references/performance.md`
- Fetching data → `references/data-fetching.md`
- Building forms → `references/forms.md`
- Working with Server Components or Actions → `references/server-rendering.md`
- Writing tests → `references/testing.md`
- Setting up routes → `references/routing.md`
- Styling → `references/styling.md`
- Handling errors → `references/error-handling.md`
- Security review → `references/security.md`
- Accessibility → `references/accessibility.md`
- Project structure → `references/project-structure.md`
- TypeScript types → `references/typescript.md`

## Principles

Check sibling files for existing patterns first — consistency beats any rule here.

- Effects are escape hatches for external system sync — never for derived state or user events. Derive state during render. Event-specific logic goes in event handlers
- Composition over configuration — compound components with shared context, explicit variants, children over render props. Never boolean prop proliferation
- State colocation — keep state as close to usage as possible. TanStack Query/SWR for async data, useState/Zustand for client state, nuqs for URL state, Context for read-heavy/write-rare only
- Never define components inside components — causes full remount every render
- Never import from barrel files — direct imports save 200-800ms
- `ErrorBoundary` wraps `Suspense`, not inside it. Granular per-section boundaries
- Test behavior, not implementation. Query priority: `getByRole` > `getByLabelText` > `getByText` > `getByTestId`
- Semantic HTML first — `<button>` not `<div role="button" tabIndex={0} onClick>`
- React auto-escapes `{value}` but NOT `dangerouslySetInnerHTML`, URLs, or `ref.innerHTML`

## React 19

- `use()` replaces `useContext()` — can be called conditionally
- `ref` is a regular prop — `forwardRef` deprecated
- `useEffectEvent` separates event logic from effect dependencies
- React Compiler auto-memoizes — manual `memo`/`useMemo`/`useCallback` often unnecessary
- `useActionState` for form state, `useFormStatus` for pending, `useOptimistic` for immediate feedback

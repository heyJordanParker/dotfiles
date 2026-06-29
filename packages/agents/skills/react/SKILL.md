---
name: react
description: "Apply this skill whenever writing, reviewing, or refactoring React code. This includes creating or modifying components, hooks, forms, data fetching, state management, routing, testing, and TypeScript patterns. Triggers for performance issues (re-renders, bundle size, waterfalls), composition patterns, effect anti-patterns, accessibility violations, security concerns, and architectural decisions. Also use for React code reviews and refactoring existing React code to follow best practices. Covers any task involving React frontend code patterns. Do NOT use for React Native mobile patterns."
user-invocable: false
---

# React Best Practices

Check sibling files for existing patterns first — consistency beats any practice here.

## Conventions

Apply to every React file:

- Component ordering: hooks → state → derived values → event handlers → early returns → render
- `use()` replaces `useContext()` — can be called conditionally (React 19)
- `ref` is a regular prop — `forwardRef` deprecated (React 19)
- `useEffectEvent` separates event logic from effect dependencies (React 19.2+)
- React Compiler auto-memoizes — manual `memo`/`useMemo`/`useCallback` often unnecessary. Follow Rules of React strictly. Use `"use no memo"` to opt out
- `<Activity mode="visible|hidden">` preserves state for hidden content (tabs, navigation)
- `0 && <Component />` renders "0" — use `count > 0 &&` or ternary
- Immutable array methods: `.toSorted()`, `.toReversed()`, `.toSpliced()`, `.with()`
- Functional setState: `setItems(curr => [...curr, newItem])` — prevents stale closures
- Module-level app init with `let didInit = false` guard — not in effects (double-fires in StrictMode)
- Named exports over default exports — grep-able, refactoring-safe
- Absolute imports via `@/` prefix — never deep `../../../` chains
- Lazy state initialization: `useState(() => expensive())` — not `useState(expensive())`

## References

Read the reference that matches the current task:

- [composition.md](references/composition.md) — Building components: compound components, variants, boolean props, context interfaces
- [state-management.md](references/state-management.md) — Managing state or writing hooks: colocation, Context, external stores, custom hook patterns
- [effects.md](references/effects.md) — Deciding whether to use an effect: 12 anti-patterns and alternatives
- [performance.md](references/performance.md) — Optimizing performance: rendering, bundle size, waterfalls
- [data-fetching.md](references/data-fetching.md) — Fetching data: TanStack Query/SWR, caching, mutations, optimistic updates
- [forms.md](references/forms.md) — Building forms: useActionState, useOptimistic, useFormStatus
- [server-rendering.md](references/server-rendering.md) — Server-side React: RSC, Server Actions, serialization, SSR patterns
- [testing.md](references/testing.md) — Writing tests: RTL query priority, user-event, MSW, async patterns
- [routing.md](references/routing.md) — Setting up routes: React Router loaders/actions, lazy routes, error handling
- [styling.md](references/styling.md) — Styling: Tailwind, CSS Modules, CSS-in-JS, dark mode
- [error-handling.md](references/error-handling.md) — Handling errors: react-error-boundary, Suspense integration, granular boundaries
- [security.md](references/security.md) — Security review: XSS, URL sanitization, CSP, auth tokens
- [accessibility.md](references/accessibility.md) — Accessibility: semantic HTML, ARIA, keyboard, focus management
- [project-structure.md](references/project-structure.md) — Project structure: feature-based org, colocation, naming, imports
- [typescript.md](references/typescript.md) — TypeScript types: discriminated unions, generics, utility types, context typing

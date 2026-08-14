---
name: react
description: "Apply this Skill when writing, reviewing, or refactoring React code. Covers components, hooks, forms, data fetching, state management, routing, testing, TypeScript patterns, performance issues, composition, effect anti-patterns, accessibility, security, Architectural Decisions, React code reviews, and React refactors. TRIGGER on React frontend code. DO NOT TRIGGER for React Native mobile patterns."
user-invocable: false
---

# React

- One Process: follow local Precedent first, apply shared React Rules, open the matching Reference, and verify the affected User behavior.

## 1. Find Precedent

Check sibling files before writing or reviewing. Local consistency beats every shared React Rule here.

## 2. Apply the shared React Rules

### Order component bodies consistently

Use hooks, then state, then derived values, then event handlers, then early returns, then render.

### Use React 19 primitives

`use()` replaces `useContext()` and may be called conditionally. `ref` is a regular prop; avoid `forwardRef`. `useEffectEvent` separates event logic from effect dependencies in React 19.2 and later.

### Let React Compiler handle memoization

React Compiler auto-memoizes. Manual `memo`, `useMemo`, and `useCallback` are often unnecessary. Follow Rules of React strictly and use `"use no memo"` to opt out.

### Preserve hidden state with Activity

Use `<Activity mode="visible|hidden">` for hidden content that should keep state, such as tabs and navigation.

### Guard conditional rendering against zero

Never: `0 && <Component />` because React renders `0`.
Example: use `count > 0 && <Component />` or a ternary.

### Update arrays immutably

Use `.toSorted()`, `.toReversed()`, `.toSpliced()`, and `.with()`.

### Use functional state updates

Example: `setItems(curr => [...curr, newItem])` prevents stale closures.

### Run application initialization at module level

Use a module-level `let didInit = false` guard instead of an effect that double-fires in Strict Mode.

### Prefer named exports

Named exports are grep-able and refactoring-safe. Avoid default exports unless a library boundary requires one.

### Prefer absolute imports

Use the `@/` prefix instead of deep `../../../` chains.

### Lazily initialize expensive state

Example: `useState(() => expensive())`.
Never: `useState(expensive())`.

## 3. Open the matching Reference

Read the Reference for the problem being solved before writing or reviewing.

## 4. Verify behavior

Exercise the affected User behavior or run the project test that covers it.

## References (each solves one problem)

- Building components → building-components.md
- Managing state or writing hooks → managing-state.md
- Deciding whether to use an effect → choosing-an-effect.md
- Optimizing performance → optimizing-performance.md
- Fetching data → fetching-data.md
- Building forms → building-forms.md
- Server-side React → rendering-on-the-server.md
- Writing tests → writing-tests.md
- Setting up routes → setting-up-routes.md
- Styling → styling-components.md
- Handling errors → handling-errors.md
- Reviewing security → reviewing-security.md
- Accessibility → making-accessible.md
- Project Architecture → structuring-the-project.md
- TypeScript types → typing-components.md

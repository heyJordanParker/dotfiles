# CSS Architecture

## CSS First, React When Necessary

CSS handles:
- Disabled states (`:disabled`, `[aria-disabled="true"]`)
- Hover/focus/active states
- Sizes and spacing
- Visibility (`[data-visible="false"]`)
- Loading states (`.loading` class)
- Transitions and animations

React handles:
- Data fetching and state
- Event handlers that change data
- Conditional rendering of different components
- Form submission logic

**Rule:** If it's visual or behavioral and can be done with CSS + data attributes, do it in CSS. React toggles classes/attributes, CSS does the rest.

```css
/* CSS handles the visual state */
.button:disabled {
  @apply opacity-50 cursor-not-allowed;
}

.sidebar[data-collapsed="true"] {
  width: var(--sidebar-width-icon);
}
```

```tsx
/* React just sets the attribute */
<button disabled={isLoading}>Submit</button>
<aside data-collapsed={isCollapsed}>...</aside>
```

## File Structure

```
styles/
├── index.css              ← Entry point, imports, @theme inline, layers
├── theme.css              ← Design system vars (copy from theme generators)
├── global.css             ← App-level styles, keyframes (keep lean)
├── components/
│   └── [component].css    ← BEM classes + component-scoped vars
├── utilities/
│   └── [utility].css      ← Utility classes (use sparingly)
└── pages/
    └── [page].css         ← Page-scoped styles + vars
```

## Core Principles

1. **Use BEM** for component classes
2. **Use Tailwind utilities sparingly** - prefer BEM classes with @apply
3. **Use @apply inside BEM classes** - keeps TSX clean
4. **DRY via CSS variables** - define once, use everywhere
5. **Scope variables appropriately** - not everything in theme.css

## Variable Scoping

Variables live where they belong:

| Scope | Location | Purpose |
|-------|----------|---------|
| Design system | `theme.css` | Colors, shadows, fonts, radii. Copy-paste from generators. |
| App-level | `global.css` | Timing, easing. Keep this lean. |
| Component | `components/*.css` | `--sidebar-width`, `--card-padding` |
| Page | `pages/*.css` | Page-specific overrides |

**Rule:** If only one component uses a variable, it lives in that component's file.

## CSS Layers

```css
@layer base, components, utilities, wordpress-fixes;
```

Specificity order: base < components < utilities < wordpress-fixes

## BEM Naming

```css
.sidebar { }                    /* Block */
.sidebar__header { }            /* Element */
.sidebar--collapsed { }         /* Modifier */
```

## Tailwind @apply Pattern

Use @apply inside BEM classes to compose Tailwind utilities:

```css
.sidebar-header {
  @apply absolute top-0 left-0 w-full z-20 p-0;
  @apply bg-sidebar-accent/30 backdrop-blur-md;
}

.sidebar-menu-btn {
  @apply w-full cursor-pointer transition-all duration-200;
}
```

**Why:**
- TSX stays clean (semantic class names, not utility soup)
- Tailwind handles the values (rem, responsive, dark mode)
- BEM provides structure and discoverability

## Native CSS Nesting

Use `&` for pseudo-classes and attribute selectors:

```css
.sidebar-menu-btn {
  @apply transition-all duration-200;

  &:hover {
    @apply bg-sidebar-accent;
  }

  &[data-active="true"] {
    @apply bg-sidebar-accent;
  }
}
```

## Tailwind v4 Gotchas

### Arbitrary values need explicit var()

```css
/* Wrong */
width: w-[--sidebar-width];

/* Right */
width: w-[var(--sidebar-width)];
```

### No BEM-style nested modifiers

Tailwind v4 doesn't support `&--modifier` syntax inside selectors.

```css
/* Wrong - won't work */
.sidebar-icon {
  &--active {
    @apply text-primary;
  }
}

/* Right - flat rule */
.sidebar-icon--active {
  @apply text-primary;
}
```

## Component-Scoped Variables

Define component-specific variables in the component file:

```css
/* components/sidebar.css */
:root {
  --sidebar-width: 16rem;
  --sidebar-width-mobile: 18rem;
  --sidebar-width-icon: 3rem;
}

.sidebar {
  width: var(--sidebar-width);
}
```

## @theme Bridge (Tailwind v4)

Expose CSS variables to Tailwind utilities in index.css:

```css
@theme inline {
  --color-primary: var(--primary);
  --color-background: var(--background);
  --shadow-sm: var(--shadow-sm);
}
```

This lets you use `bg-primary`, `shadow-sm` in @apply and TSX.

## File Naming

- No underscores (not `_sidebar.css`)
- Lowercase kebab-case (`sidebar-menu.css`)
- Match component/page name exactly

## Units

**Use rem.** Tailwind uses rem natively. Divide px by 16.

| px | rem |
|----|-----|
| 12 | 0.75rem |
| 14 | 0.875rem |
| 16 | 1rem |
| 18 | 1.125rem |

Rem respects user browser font settings (accessibility).

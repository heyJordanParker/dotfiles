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

- **Design system:** `theme.css` — Colors, shadows, fonts, radii. Copy-paste from generators.
- **App-level:** `global.css` — Timing, easing. Keep this lean.
- **Component:** `components/*.css` — `--sidebar-width`, `--card-padding`
- **Page:** `pages/*.css` — Page-specific overrides

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

### The Layout-Component Mix Rule

Avoid deep nesting ("grandchildren") and coupling by strictly separating **Layout** (positioning) from **Component** (appearance).

**The Rules:**

1. **2-Level Limit:** Never create `block__element__sub-element`. If you need a third level, start a new Block.

2. **The Mix:** When a structural slot contains a UI component, apply **two classes** to the same DOM element:
   - **Layout Role:** `parent-block__element` → `grid-area`, `margin`, `z-index`, `position`
   - **Component Role:** `child-block` → `background`, `border`, `padding`, `display: flex/gap`

3. **Separation of Concerns:**
   - Parent Block knows *where* children sit, not *what* they look like
   - Child Block knows *how* it looks, not *where* it sits

**Wrong:**
```html
<div class="frame">
  <div class="frame__footer">
    <button class="frame__footer-btn">Save</button>
  </div>
</div>
```

**Right (The Mix):**
```html
<div class="frame">
  <div class="frame__dock toolbar">
    <button class="toolbar__btn">Save</button>
  </div>
</div>
```

```css
/* Layout: where it sits */
.frame__dock {
  grid-area: footer;
  margin-block-start: auto;
}

/* Component: how it looks */
.toolbar {
  @apply flex gap-2 p-2 bg-muted border-t;
}
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

## Sizing Hierarchy

Use units in priority order:

1. **Token sizing** - Project tokens first (auto-clamped via clamp())
2. **Grid/flex sizing** - Every block is grid/flex. Always add 1 dynamic-width column. Fixed widths use `ch` or `%`, never `px`
3. **Typography sizing** - `1em`, `1lh`, `1rlh` for rhythm. Icons, avatars, buttons can use these
4. **Container sizing** - `cqi`, `cqb` over media queries and viewport units
5. **Visual alignment** - Sub-em offsets (`0.02em`) for optical corrections. Also for letter-spacing

**Hard Rules:**

- **Never use pixels** - except borders (aliasing) and box-shadow (constant depth across sizes)
- **Never use margin for offsets** - use gaps. Negative margins allowed to elegantly remove gaps
- **Use `calc()` sparingly** - 80% of content uses default tokens to establish rhythm
- **Paragraphs ≤55ch** - long lines overwhelm users on wide displays

**Layout pattern:**

```css
.container {
  display: grid;
  grid-template-columns: 1fr auto 1fr; /* Always include dynamic column */
  gap: 1rem;
}
```

**Typography rhythm:**

```css
.icon {
  width: 1em;
  height: 1em;
}

.avatar {
  width: 2lh;  /* 2x line height */
  height: 2lh;
}

.button {
  padding-block: 0.5lh;
  padding-inline: 1em;
}
```

**Visual alignment (optical corrections):**

```css
.icon-with-text {
  margin-top: 0.02em; /* Align icon baseline with text */
}

.heading {
  letter-spacing: -0.02em; /* Tighten large text */
}
```

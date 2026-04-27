---
name: design
description: Use when building UI components, styling pages, or making visual/interaction decisions. Guides design thinking, anti-patterns, CSS architecture, and responsive patterns.
---

# Frontend Design

## Design Modes

- **minimal** (default): Less is more, function-first — Admin tools, utilities, dashboards
- **bold**: Distinctive, unforgettable — Landing pages, portfolios, marketing

For bold mode, see [bold.md](references/bold.md).

## Standard Operating Procedure

### Step 1: Think (Before Code)

**Good design is as little design as possible.**

Ask: What's the key functionality? Start from there. Fewer colors, fewer words, less clutter.

1. **Purpose** - What does this accomplish for the user?
2. **Tone** - Professional? Playful? Minimal? Bold?
3. **Constraints** - Space available? Existing patterns to match?
4. **Differentiation** - What makes this feel intentional, not generic?

**Intentionality > Intensity.** Every choice should have a reason.

### Step 2: Build

**Anti-Patterns — never create generic AI aesthetics:**
- Gaudy, high-saturation, or rainbow gradients (subtle gradients that add texture are good)
- Excessive rounded corners on everything
- Purple-blue-pink color schemes with no purpose
- Animations that don't serve function
- One-sided colored borders as accent indicators (`border-left: 3px solid primary`) — use spacing, background, or typography weight to distinguish elements
- Divider lines (`<hr>`, `border-top/bottom` separators) to separate content sections — use hierarchy (spacing, size, weight) instead. Dividers are only appropriate inside accordions or expandable elements where they separate togglable items
- Relying on lines to organize instead of designing with hierarchy — if you need a line to show where one section ends and another begins, the spacing, sizing, or weight difference between sections is insufficient

If it looks like every AI-generated landing page, redo it.

**Core Principles:**
- **Cutting-edge CSS** — Gaps over margins, view transitions over JS animations, logical properties over directional. Use modern CSS when universally supported
- **Simplicity wins** — Remove until it breaks, then add one thing back
- **Hierarchy through restraint** — One focal point per view
- **Function drives form** — Every visual choice serves usability
- **Consistency > novelty** — Match existing patterns before inventing
- **Encapsulated and reusable** — Components work outside their current context

#### CSS Ownership

**Rule:** If it's visual or behavioral and can be done with CSS + data attributes, do it in CSS. React toggles classes/attributes, CSS does the rest.

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

```css
.button:disabled {
  @apply opacity-50 cursor-not-allowed;
}
.sidebar[data-collapsed="true"] {
  width: var(--sidebar-width-icon);
}
```

```tsx
<button disabled={isLoading}>Submit</button>
<aside data-collapsed={isCollapsed}>...</aside>
```

**BEM naming** — Block, Element, Modifier. 2-level max — never `block__element__sub-element`, start a new block instead.

```css
.sidebar { }                    /* Block */
.sidebar__header { }            /* Element */
.sidebar--collapsed { }         /* Modifier */
```

**@apply inside BEM classes** — TSX has semantic names, not utility soup. Tailwind handles values (rem, responsive, dark mode), BEM provides structure and discoverability.

```css
.sidebar-header {
  @apply absolute top-0 left-0 w-full z-20 p-0;
  @apply bg-sidebar-accent/30 backdrop-blur-md;
}
.sidebar-menu-btn {
  @apply w-full cursor-pointer transition-colors duration-200;
}
```

Arbitrary Tailwind values use explicit `var()` — `w-[var(--x)]` not `w-[--x]`.

Tailwind v4 doesn't support `&--modifier` nesting — use flat rules:

```css
/* Wrong */
.sidebar-icon { &--active { @apply text-primary; } }
/* Right */
.sidebar-icon--active { @apply text-primary; }
```

**Native CSS nesting** — use `&` for pseudo-classes and attribute selectors:

```css
.sidebar-menu-btn {
  @apply transition-colors duration-200;
  &:hover { @apply bg-sidebar-accent; }
  &[data-active="true"] { @apply bg-sidebar-accent; }
}
```

**Encapsulated** — components work outside their current context. No assumptions about parent layout.

**Reuse over creation** — match existing patterns before inventing. Extract reusable CSS classes for patterns that obviously recur.

**Variable scoping** — variables live where they belong:
- **Design system:** `theme.css` — colors, shadows, fonts, radii
- **App-level:** `global.css` — timing, easing (keep lean)
- **Component:** `components/*.css` — `--sidebar-width`, `--card-padding`
- **Page:** `pages/*.css` — page-specific overrides

If only one component uses a variable, it lives in that component's file.

```css
/* components/sidebar.css */
:root {
  --sidebar-width: 16rem;
  --sidebar-width-mobile: 18rem;
  --sidebar-width-icon: 3rem;
}
.sidebar { width: var(--sidebar-width); }
```

**Layout-Component Mix** — avoid deep nesting and coupling by separating Layout (positioning) from Component (appearance). When a structural slot contains a UI component, apply two classes to the same element:
- Layout role (`parent-block__element`): `grid-area`, `place-self`, `z-index`, `position`
- Component role (`child-block`): `background`, `border`, `padding`, `display: flex/gap`

Parent knows *where* children sit, not *what* they look like. Child knows *how* it looks, not *where* it sits.

```html
<!-- Wrong: grandchild selector -->
<div class="frame">
  <div class="frame__footer">
    <button class="frame__footer-btn">Save</button>
  </div>
</div>

<!-- Right: the Mix -->
<div class="frame">
  <div class="frame__dock toolbar">
    <button class="toolbar__btn">Save</button>
  </div>
</div>
```

```css
/* Layout: where it sits */
.frame__dock { grid-area: footer; place-self: end; }
/* Component: how it looks */
.toolbar { @apply flex gap-2 p-2 bg-muted border-t; }
```

#### Layout

**Every block uses grid or flex.** Always include at least one dynamic-width column. Use `gap` for all spacing.

**Never use margins for layout spacing.** Margins only for:
- Sub-em optical corrections (`0.02em` to align baselines)
- `auto` margins for flex/grid alignment
- Negative margins to elegantly remove gaps

```css
.container {
  display: grid;
  grid-template-columns: 1fr auto 1fr; /* Always include dynamic column */
  gap: 1rem;
}
```

**Container queries over media queries.** Container queries ask "how much room do I have?" instead of "how big is the screen?" — components adapt to their context, not device size.

**Use container queries for:** cards, grids, forms, galleries, tables, headers, footers, navigation — any component that could exist in multiple contexts.

**Use media queries only for:** modals, off-canvas menus, fixed-position elements, device-specific concerns (print, reduced-motion).

Auto-declare container from the child with the "Has Me" pattern:

```css
.card {
  /* Auto-declare parent as container */
  :has(> &) { container-type: inline-size; }

  /* Now use container queries */
  @container (min-width: 400px) {
    display: grid;
    grid-template-columns: 200px 1fr;
  }
}
```

**Grid items need physical cells** for container queries to measure. Use semantic wrappers:

```html
<ul class="grid">
  <li>
    <article class="card">...</article>
  </li>
</ul>
```

```css
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 1rem;
}
.grid > li { container-type: inline-size; }
.card {
  @container (min-width: 300px) { /* horizontal layout */ }
}
```

**Container units:**
- `cqi` — 1% of container's inline size (width)
- `cqb` — 1% of container's block size (height)
- `cqmin` / `cqmax` — smaller/larger of the two

Prefer `cqi`/`cqb` over viewport units (`vw`/`vh`).

**Touch detection** — desktop-only hover effects:

```css
@media (hover: hover) {
  .card:hover { @apply shadow-md -translate-y-1; }
}
```

Touch targets: minimum 44x44px (see hit area technique in [interactable.md](references/interactable.md)).

**When media queries are appropriate:**

```css
@media print {
  .no-print { display: none; }
}
@media (prefers-reduced-motion: reduce) {
  * { animation-duration: 0.01ms !important; }
}
/* Off-canvas menu — device-specific */
@media (max-width: 768px) {
  .sidebar { transform: translateX(-100%); }
}
```

#### Sizing

Use units in priority order:

1. **Design tokens** — auto-clamped via `clamp()`
2. **Grid/flex** — `ch` or `%` for fixed widths, never `px`
3. **Typography** — `1em`, `1lh`, `1rlh` for rhythm. Icons, avatars, buttons can use these
4. **Container** — `cqi`, `cqb` over media queries and viewport units
5. **Sub-em** — optical corrections (`0.02em`) and letter-spacing

**No pixels** except borders (aliasing) and box-shadow (constant depth). All sizes in `rem` — users can adjust browser font size, rem respects this, px doesn't.

Use `calc()` sparingly — 80% of content uses default tokens to establish rhythm.

```css
.icon { width: 1em; height: 1em; }
.avatar { width: 2lh; height: 2lh; /* 2x line height */ }
.button { padding-block: 0.5lh; padding-inline: 1em; }
```

#### Spacing

**4px grid in rem:**

- **0.25rem** (4px) / `gap-1`, `p-1`
- **0.5rem** (8px) / `gap-2`, `p-2`
- **0.75rem** (12px) / `gap-3`, `p-3`
- **1rem** (16px) / `gap-4`, `p-4`
- **1.25rem** (20px) / `gap-5`, `p-5`
- **1.5rem** (24px) / `gap-6`, `p-6`
- **2rem** (32px) / `gap-8`, `p-8`
- **2.5rem** (40px) / `gap-10`, `p-10`
- **3rem** (48px) / `gap-12`, `p-12`
- **4rem** (64px) / `gap-16`, `p-16`

**Use Tailwind utilities** for spacing: `gap-4`, `p-6`. Define CSS variables only for component-specific values.

**Process:** Start generous (2.5rem / 40px), bring elements closer until they feel grouped, pick from scale.

**Vertical spacing: rem. Horizontal sizing: ch.**
- `rem` for padding, gaps (scales with root font)
- `ch` for widths of text containers, inputs, content areas (scales with font)

```css
.layout {
  container-type: inline-size;
  display: grid;
  grid-template-columns: 1fr 40ch; /* Main + sidebar */
  @container (width < 100ch) {
    grid-template-columns: 1fr;
  }
}
```

#### Typography

**3 sizes only (rem):**
- **0.75rem** (12px / `text-xs`) — Captions, metadata
- **0.875rem** (14px / `text-sm`) — Body text, UI (base)
- **1.125rem** (18px / `text-lg`) — Headings, emphasis

**Hierarchy through weight and color, not size.** To emphasize an element, de-emphasize everything else. You can't make white "more white" — instead reduce the lightness of secondary text.

- **Primary (titles):** 90-100% lightness / `text-foreground`
- **Secondary:** 60-70% lightness / `text-muted-foreground`
- **Disabled/hint:** 40-50% lightness / `text-muted-foreground/50`

**Text wrapping:**
- `text-wrap: balance` — headings and short text (≤6 lines in Chromium, ≤10 in Firefox). Distributes text evenly, prevents orphans. Computationally expensive — browsers limit to short text.
- `text-wrap: pretty` — body paragraphs. Optimizes last line to avoid orphans without the line limit. Use on longer text where `balance` would be silently ignored.
- Neither — code blocks, pre-formatted text.

**Font smoothing:** macOS default subpixel antialiasing renders text heavier than intended. Apply `antialiased` (Tailwind) or `-webkit-font-smoothing: antialiased` to the layout root.

**Tabular numbers:** `tabular-nums` for any numbers that update dynamically (counters, prices, tables). Without it, digits have variable width and visually shift on every update. Note: some fonts like Inter change numeral appearance when this is applied.

**Line length:** Max 55ch for paragraphs — long lines overwhelm users on wide displays.

**Line height as spacing:** Greater line height acts as natural margin-bottom. In most cases you don't need manual gap between text blocks.

#### Colors (OKLCH)

OKLCH is perceptually uniform — colors with same lightness actually look equally bright.

**Never hardcode colors.** Always derive from tokens:

```css
/* Wrong */
background: #3b82f6;
background: oklch(0.64 0.17 250);

/* Right — derive from tokens */
background: var(--primary);
background: color-mix(in oklch, var(--primary), transparent 50%);
background: oklch(from var(--primary) l c h / 50%);
```

**Token format:**
```css
--primary: oklch(0.64 0.17 36);  /* oklch(lightness chroma hue) */
```

**Color mixing patterns:**
```css
color-mix(in oklch, var(--color), transparent 50%)  /* Transparency */
oklch(from var(--color) calc(l + 0.1) c h)          /* Lighten */
oklch(from var(--color) calc(l - 0.1) c h)          /* Darken */
oklch(from var(--color) l calc(c * 0.5) h)          /* Desaturate */
```

**Palette generation with 60° hue shifts** — creates a 120° arc, same distance between primary colors on the wheel:
```css
:root {
  --hue: 36;
  --primary: oklch(0.64 0.17 var(--hue));
  --secondary: oklch(0.55 0.12 var(--hue));
  --tertiary: oklch(0.64 0.15 calc(var(--hue) - 60));  /* 60° left */
  --accent: oklch(0.64 0.15 calc(var(--hue) + 60));    /* 60° right */
}
```

**Dark/light mode:** `Light mode lightness = 100 - Dark mode lightness`

**Hierarchy:**
- Background: Very low chroma (nearly gray)
- Text: No chroma (pure gray) or very low
- Accents: Higher chroma for emphasis

#### Gradients

Never gaudy. Use gradients to add texture and make UI feel less flat — keep subtle and elegant.

**Oklab interpolation:** Tailwind v4 uses oklab by default — smoother than sRGB, no muddy midpoints.

**Grain overlay** — breaks digital smoothness:
```css
.hero {
  @apply bg-gradient-to-br from-slate-900 via-slate-800 to-slate-900;
  position: relative;
}
.hero::after {
  content: '';
  @apply absolute inset-0 pointer-events-none;
  background: url('/noise.svg');
  opacity: 0.03;
}
```

**Subtle card accent:**
```css
.card--elevated {
  @apply bg-gradient-to-b from-white/5 to-transparent;
}
```

#### Shadows & Elevation

**Light source is at the top.** Top surfaces are lighter, bottom surfaces are darker.

- **Level 0:** Page base / no shadow — Content areas
- **Level 1:** Slightly lifted / `shadow-xs` — Sidebar body, cards
- **Level 2:** Floating / `shadow-sm` — Sticky headers, glass panels
- **Level 3:** Overlay / `shadow-md` — Dropdowns, modals

**Dual shadow system (soft + dark)** — combine two shadow types for realistic depth:
1. Light edge on top — simulates light hitting elevated surface
2. Dark shadow at bottom — the actual shadow cast

```css
box-shadow:
  inset 0 1px 0 rgba(255,255,255,0.05),  /* Light edge top */
  0 4px 12px rgba(0,0,0,0.03),            /* Soft ambient */
  0 1px 3px rgba(0,0,0,0.06);             /* Sharp contact */
```

**Recessed elements** (inputs, wells): dark inset shadow on top + light inset shadow on bottom.

**Shadows instead of borders** — solid border colors don't adapt to varied backgrounds (images, gradients). Use multi-layer `box-shadow` for border-like definition that works universally via transparency:

```css
/* Wrong — solid border breaks on non-white backgrounds */
border: 1px solid #e5e7eb;

/* Right — shadow adapts to any background */
box-shadow:
  0px 0px 0px 1px rgba(0,0,0,0.06),
  0px 1px 2px -1px rgba(0,0,0,0.06),
  0px 2px 4px 0px rgba(0,0,0,0.04);

/* Hover — same shadows, slightly darker */
box-shadow:
  0px 0px 0px 1px rgba(0,0,0,0.08),
  0px 1px 2px -1px rgba(0,0,0,0.08),
  0px 2px 4px 0px rgba(0,0,0,0.06);

/* Dark mode — simplify to a single white ring.
   Layered depth shadows are invisible against dark backgrounds. */
--shadow-border: 0 0 0 1px rgba(255,255,255,0.08);
--shadow-border-hover: 0 0 0 1px rgba(255,255,255,0.13);
```

Transition between states with `transition-[box-shadow]`.

**Image outlines** — images can blend into surrounding content when edge colors match background:
```css
.image-outline {
  outline: 1px solid rgba(0,0,0,0.1);
  outline-offset: -1px;
}
.dark .image-outline {
  outline-color: rgba(255,255,255,0.1);
}
```

**Rules:**
- Elevated = lighter background + more shadow
- Never use z-index without corresponding shadow
- Glass effect: `backdrop-blur-md` + semi-transparent bg + layered shadow

#### Radius

One radius value: `--radius: 0.675rem`

Variants derived from it:
- Small elements: `calc(var(--radius) - 0.125rem)`
- Large containers: `calc(var(--radius) + 0.25rem)`

Don't round everything. Intentional use only.

**Concentric border radius:** outer = inner + padding. Mismatched radii create a subtle visual imbalance. Exception: if padding > 24px, treat layers as separate surfaces.

```css
/* Wrong — same radius on both */
.outer { border-radius: 12px; padding: 8px; }
.inner { border-radius: 12px; }

/* Right — outer = inner + padding */
.outer { border-radius: 20px; padding: 8px; }
.inner { border-radius: 12px; }
```

#### Optical Corrections

Geometric centering can look visually off because shapes have different visual weight. Buttons with text + icon need smaller padding on the icon side to look balanced:

```css
/* Wrong — equal padding looks off because icon has built-in whitespace */
.button-with-icon { padding-inline: 1em; }

/* Right — asymmetric padding, less on icon side */
.button-with-icon {
  padding-inline-start: 0.75em;
  padding-inline-end: 1em;
}
```

Play button triangles — geometric center sits left of the visual center because most of the shape's mass is on the left:
```css
.play-button svg { margin-left: 2px; }
```

Best fix for icons: adjust the whitespace in the SVG itself so no extra CSS is needed.

**Visual alignment offsets:**
```css
.icon-with-text { margin-top: 0.02em; /* Align icon baseline with text */ }
.heading { letter-spacing: -0.02em; /* Tighten large text */ }
```

#### Contrast & Accessibility

**Minimum contrast ratios:**
- Normal text: 4.5:1
- Large text (1.125rem+ bold or 1.5rem+): 3:1
- UI components: 3:1

**OKLCH shortcut:** Lightness difference of ~0.4 usually passes.

**Use rem for accessibility.** Users can adjust browser font size — rem respects this, px doesn't.

#### Transitions

State changes need motion. Without it, the user sees the *result* but misses the *change* — they don't track what moved, what appeared, what was theirs and what was the system's. Transitions communicate causality. A modal that pops into existence reads as broken. A dropdown that snaps closed instead of closing feels cheap. A counter that jumps from `12.34` to `15.67` looks like a bug.

Three classes, each with a different mandate:

- **Reactive** — element responds to user input (hover, press, focus). 100–200ms, interruptible (CSS transitions, never keyframes).
- **State change** — element opens / closes / swaps / reveals. 200–400ms, scales with element size, asymmetric open/close. Open scales up from a pre-scale; close scales outward and fades. Mirroring enter/exit feels mechanical.
- **Navigational** — view changes (route, page). View Transitions API; never JS-driven libraries.

Transitions are mandatory for state changes. Skipping is a *design decision* (`prefers-reduced-motion`, high-frequency interactions seen 100+ times daily) — never the default and never the lazy answer.

Two anti-defaults to watch for, both common agent reaches:

- **JavaScript animation libraries** (Framer Motion `<motion.span key={value}>` for text swaps, `react-spring` counters, `useResizeObserver` + RAF for resize). Modern CSS handles all three with class toggles + intrinsic property transitions. Only reach for JS when the motion is genuinely physics-driven (drag, gestures with release velocity).
- **`transform: scale()` for resize**. Distorts content. Animate intrinsic `width` / `height`.

For timing tables, easing curves, interruptibility rules, micro-interactions, and the View Transitions API: see [motion.md](references/motion.md).

For state-change recipes (reveals, content swaps, in-component page transitions, animated counters, origin-anchored menus): see [transitions.md](references/transitions.md).

#### CSS File Structure

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

**Layers:**
```css
@layer base, components, utilities, wordpress-fixes;
```
Specificity order: base < components < utilities < wordpress-fixes

**File naming:** no underscores, lowercase kebab-case, match component/page name.

**@theme bridge (Tailwind v4)** — expose CSS variables to Tailwind utilities in index.css:
```css
@theme inline {
  --color-primary: var(--primary);
  --color-background: var(--background);
  --shadow-sm: var(--shadow-sm);
}
```

This lets you use `bg-primary`, `shadow-sm` in @apply and TSX.

### Step 3: Review (Required)

Run through after all design work. Every item is yes/no. Do not skip.

#### CSS Architecture
- [ ] All classes use BEM naming (block, element, modifier)
- [ ] No inline styles, style objects, or conditional className string-building for visual concerns
- [ ] React only toggles data attributes and classes — CSS handles all visual states
- [ ] No grandchildren selectors (`block__element__sub-element`) — start a new block instead
- [ ] Layout-component mix applied where structural slots contain UI components
- [ ] Variables scoped correctly (component vars in component file, not theme.css)
- [ ] @apply used inside BEM classes — TSX has semantic names, not utility soup
- [ ] Arbitrary Tailwind values use explicit `var()` — `w-[var(--x)]` not `w-[--x]`

#### Sizing & Layout
- [ ] No pixel values (except borders and box-shadow)
- [ ] Every block uses grid or flex with at least one dynamic-width column
- [ ] Gaps used instead of margins for spacing
- [ ] Fixed widths use `ch` or `%`, never `px`
- [ ] Typography-based sizing where appropriate (`em`, `lh` for icons, avatars, buttons)
- [ ] Paragraphs max 55ch line length

#### Visual Design
- [ ] Spacing values from the 4px grid scale
- [ ] Only 3 text sizes (0.75, 0.875, 1.125rem) — hierarchy via weight/color, not size
- [ ] No hardcoded colors — all derived from tokens via `var()` or `color-mix(in oklch, ...)`
- [ ] Font smoothing applied to root (`antialiased`)
- [ ] Headings use `text-wrap: balance`, body text uses `text-wrap: pretty`
- [ ] Dynamic numbers use `tabular-nums`
- [ ] Nested rounded elements use concentric border radius (outer = inner + padding; independent if padding > 24px)
- [ ] Icons optically centered — asymmetric padding on icon buttons, play triangles shifted right
- [ ] Shadows used instead of borders where elements need depth on varied backgrounds
- [ ] Dark mode shadows simplified to single white ring (not multi-layer)
- [ ] Images have subtle semi-transparent outline
- [ ] No z-index without corresponding shadow

#### Interactions & Animation
- [ ] No `transition: all` or bare Tailwind `transition` — only specific properties
- [ ] `will-change` only on `transform`, `opacity`, `filter`, `clip-path` — never `all`
- [ ] Exits roughly half the duration of entrances, more subtle (fixed `-12px`, not full height)
- [ ] Enter animations staggered where multiple elements appear (~100ms between groups)
- [ ] Interactive elements use CSS transitions (interruptible), keyframes only for one-shot sequences
- [ ] Button press uses `scale(0.96)` on `:active` where appropriate (never < `0.95`)
- [ ] `AnimatePresence` uses `initial={false}` for default-state elements
- [ ] Icon swaps animated with opacity + scale(0.25) + blur(4px)
- [ ] View Transitions API used for page/state changes
- [ ] Micro-interactions limited to 1-2 per view
- [ ] Easing chosen by intent — default `var(--ease)`, `ease-out` for enter/exit, `ease-in-out` for on-screen motion, `ease` for hover/color, spring for drag
- [ ] High-frequency interactions (seen 100+/day) not animated
- [ ] Drag or interruptible gestures use spring, not timed transitions
- [ ] Duration scales with element size (drawers, modals ≥ 30rem use 1.3–1.5× base)

#### Responsive & Accessibility
- [ ] Container queries over media queries for component adaptation
- [ ] Media queries only for device-specific (modals, off-canvas, print, reduced-motion)
- [ ] Interactive elements have at least 44x44px hit area (pseudo-element extension if smaller)
- [ ] Extended hit areas don't overlap between adjacent elements
- [ ] Visible `focus-visible` ring on all interactive elements
- [ ] `prefers-reduced-motion` respected
- [ ] Semantic HTML (button not div, proper heading hierarchy)
- [ ] All inputs have labels, `aria-label` when visual label missing
- [ ] Minimum contrast ratios met (4.5:1 normal, 3:1 large/UI)
- [ ] All sizes in `rem`
- [ ] Desktop-only hover effects gated with `@media (hover: hover)`

#### UX Patterns
- [ ] Forms: single column, labels above inputs, validate on blur
- [ ] Error messages near the source with recovery action
- [ ] Loading states present — user never wonders if something is happening
- [ ] Empty states provide explanation + action
- [ ] Destructive actions require confirmation (safe action is primary button)
- [ ] Non-destructive actions undoable (toast with Undo), not confirmed
- [ ] Modals trap focus, close on Escape, return focus on close, prevent body scroll

#### Anti-Patterns (Reject If Present)
- [ ] No gaudy/high-saturation/rainbow gradients
- [ ] No excessive rounded corners on everything
- [ ] No purple-blue-pink color scheme without purpose
- [ ] No animations that don't serve function
- [ ] No one-sided colored borders (accent borders on left/right/top/bottom of an element)
- [ ] No divider lines between content sections — separation via spacing, size, and weight only (exception: inside accordions/expandable elements)

## References

- [motion.md](references/motion.md) - Timing, easing, states, transitions, micro-interactions
- [transitions.md](references/transitions.md) - State-change recipes: reveals, swaps, in-component page transitions, asymmetric open/close
- [interactable.md](references/interactable.md) - Forms, navigation, feedback, accessibility, modals
- [bold.md](references/bold.md) - Bold design mode

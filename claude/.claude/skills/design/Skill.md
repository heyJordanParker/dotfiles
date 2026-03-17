---
name: design
description: Use when building UI components, styling pages, or making visual/interaction decisions. Guides design thinking, anti-patterns, CSS architecture, and responsive patterns.
---

# Frontend Design

Use when building UI components, styling pages, or making visual/interaction decisions.

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

### Step 2: Build (Reference-Driven)

Consult the reference files below while implementing. Follow every principle — they are not suggestions.

**Anti-Patterns — never create generic AI aesthetics:**
- Gaudy, high-saturation, or rainbow gradients (subtle gradients that add texture are good)
- Excessive rounded corners on everything
- Purple-blue-pink color schemes with no purpose
- Animations that don't serve function
- "Modern" for modern's sake

If it looks like every AI-generated landing page, redo it.

**Core Principles:**
- **Cutting-edge CSS** - Gaps over margins, view transitions over JS animations, logical properties over directional. Use modern CSS when universally supported
- **Encapsulated and reusable** - Components work outside their current context. No assumptions about parent layout
- **BEM everywhere** - Block, Element, Modifier. No bare class names, no utility-only components. See [css-architecture.md](references/css-architecture.md)
- **Styling lives in CSS** - Never style in React. React toggles data attributes and classes. CSS handles all visual states
- **Proactive reuse** - Extract reusable CSS classes for patterns that will obviously recur
- **Simplicity wins** - Remove until it breaks, then add one thing back
- **Hierarchy through restraint** - One focal point per view
- **Consistency > novelty** - Match existing patterns before inventing
- **Function drives form** - Every visual choice serves usability

**Quick Reference:**

**Sizing:** Tokens → Grid/flex (`ch`/`%`) → Typography (`em`/`lh`) → Container (`cqi`/`cqb`) → Visual (sub-em)

**Layout:** Every block is grid/flex. Always include 1 dynamic column. Use gaps, not margins.

**Responsive:** Container queries (`cqi`) over media queries. Media queries only for device-specific (modals, off-canvas).

**Colors:** Never hardcode. Use tokens + `color-mix(in oklch, ...)` for variations.

**Spacing:** 4px grid in rem (0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 2.5, 3, 4rem)

**Typography:** 3 sizes (0.75, 0.875, 1.125rem) + weight/color for hierarchy. Max 55ch line length.

**Transitions:** Use the View Transitions API for page/state changes. See [interactions.md](references/interactions.md) for timing, easing, states, view transitions, and micro-interactions.

### Step 3: Review (Required)

Run through after all design work. Every item is yes/no. Do not skip this step.

### CSS Architecture

- [ ] All classes use BEM naming (block, element, modifier)
- [ ] No inline styles, style objects, or conditional className string-building for visual concerns
- [ ] React only toggles data attributes and classes — CSS handles all visual states
- [ ] No grandchildren selectors (`block__element__sub-element`) — start a new block instead
- [ ] Layout-component mix applied where structural slots contain UI components
- [ ] Variables scoped correctly (component vars in component file, not theme.css)
- [ ] @apply used inside BEM classes — TSX has semantic names, not utility soup
- [ ] Arbitrary Tailwind values use explicit `var()` — `w-[var(--x)]` not `w-[--x]`

### Sizing & Layout

- [ ] No pixel values (except borders and box-shadow)
- [ ] Every block uses grid or flex with at least one dynamic-width column
- [ ] Gaps used instead of margins for spacing
- [ ] Fixed widths use `ch` or `%`, never `px`
- [ ] Typography-based sizing where appropriate (`em`, `lh` for icons, avatars, buttons)
- [ ] Paragraphs max 55ch line length

### Visual Design

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

### Interactions & Animation

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

### Responsive & Accessibility

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

### UX Patterns

- [ ] Forms: single column, labels above inputs, validate on blur
- [ ] Error messages near the source with recovery action
- [ ] Loading states present — user never wonders if something is happening
- [ ] Empty states provide explanation + action
- [ ] Destructive actions require confirmation (safe action is primary button)
- [ ] Non-destructive actions undoable (toast with Undo), not confirmed
- [ ] Modals trap focus, close on Escape, return focus on close, prevent body scroll

### Anti-Patterns (Reject If Present)

- [ ] No gaudy/high-saturation/rainbow gradients
- [ ] No excessive rounded corners on everything
- [ ] No purple-blue-pink color scheme without purpose
- [ ] No animations that don't serve function

## References

- [css-architecture.md](references/css-architecture.md) - File structure, BEM, Tailwind @apply, variable scoping
- [visual-design.md](references/visual-design.md) - Colors (OKLCH), spacing, typography, shadows, elevation
- [interactions.md](references/interactions.md) - Timing, easing, states, keyframes, micro-interactions
- [ux-patterns.md](references/ux-patterns.md) - Forms, navigation, feedback, accessibility, modals
- [responsive.md](references/responsive.md) - Desktop-first breakpoints, touch detection, admin patterns

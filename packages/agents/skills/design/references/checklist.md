# Design Verification

Run after all design work. Every item is yes or no.

## 1. Verify CSS Architecture

### Check that visual behavior lives in CSS
- [ ] All classes use block element modifier naming.
- [ ] No inline styles, style objects, or conditional `className` string-building for visual concerns.
- [ ] React only toggles data attributes and classes; CSS handles all visual states.
- [ ] No grandchild selectors such as `block__element__sub-element`; a new block starts instead.
- [ ] Layout and component classes are mixed where structural slots contain UI components.
- [ ] Variables are scoped correctly: component variables in the component file, not `theme.css`.
- [ ] `@apply` is inside block element modifier classes; TSX has semantic class names, not utility soup.
- [ ] Arbitrary Tailwind values use explicit `var()`, such as `w-[var(--x)]`, never `w-[--x]`.

## 2. Verify sizing and layout

### Check that units scale with the User's environment
- [ ] No pixel values except borders and box-shadow.
- [ ] Every block uses grid or flex with at least one dynamic-width column.
- [ ] Gaps replace margins for layout spacing.
- [ ] Fixed widths use `ch` or `%`, never `px`.
- [ ] Typography-based sizing is used where appropriate: `em` and `lh` for icons, avatars, and buttons.
- [ ] Paragraphs stay at 55ch maximum line length.

## 3. Verify visual design

### Check that hierarchy comes from the system
- [ ] Spacing values come from the 4px grid scale.
- [ ] Text uses only `0.75rem`, `0.875rem`, and `1.125rem`; hierarchy comes from weight and color, not extra sizes.
- [ ] Colors derive from tokens via `var()` or `color-mix(in oklch, ...)`; no hardcoded colors.
- [ ] Font smoothing is applied to the root with `antialiased`.
- [ ] Headings use `text-wrap: balance`; body text uses `text-wrap: pretty`.
- [ ] Dynamic numbers use `tabular-nums`.
- [ ] Nested rounded elements use concentric radius: outer equals inner plus padding; independent when padding is greater than 24px.
- [ ] Icons are optically centered with asymmetric padding on icon buttons and right-shifted play triangles.
- [ ] Surface elevation uses `var(--shadow-subtle)`, `var(--shadow-elevated)`, `var(--shadow-floating)`, or `var(--shadow-extruded)`.
- [ ] Shadows replace borders where elements need depth on varied backgrounds.
- [ ] Dark mode shadows collapse to a single white ring.
- [ ] Images have a subtle semi-transparent outline.
- [ ] No `z-index` appears without a corresponding shadow tier.

## 4. Verify interactions and animation

### Check that motion is specific and interruptible
- [ ] No `transition: all` or bare Tailwind `transition`; only specific properties transition.
- [ ] `will-change` appears only on `transform`, `opacity`, `filter`, or `clip-path`, never `all`.
- [ ] Exits are roughly half the duration of entrances and more subtle, using fixed small movement such as `-12px`, not full height.
- [ ] Enter animations are staggered when multiple elements appear, about 100ms between groups.
- [ ] Interactive elements use CSS transitions; keyframes are limited to one-shot sequences.
- [ ] Button press uses `scale(0.96)` on `:active` where appropriate, never below `0.95`.
- [ ] `AnimatePresence` uses `initial={false}` for default-state elements.
- [ ] Icon swaps animate with opacity, `scale(0.25)`, and `blur(4px)`.
- [ ] The View Transitions API handles page or route changes.
- [ ] Micro-interactions are limited to one or two per view.
- [ ] Easing matches intent: `var(--ease)` by default, `ease-out` for enter and exit, `ease-in-out` for on-screen motion, `ease` for hover and color, spring for drag.
- [ ] Interactions seen 100 or more times per day are not animated.
- [ ] Drag or interruptible gestures use spring motion, not timed transitions.
- [ ] Duration scales with element size; drawers and modals at least 30rem use 1.3 to 1.5 times the base duration.

## 5. Verify responsive behavior and accessibility

### Check that components adapt and remain usable
- [ ] Container queries adapt components.
- [ ] Media queries are limited to device concerns: modals, off-canvas menus, print, and reduced motion.
- [ ] Interactive elements have at least a 44 by 44px hit area, extended with a pseudo-element if smaller.
- [ ] Extended hit areas do not overlap between adjacent elements.
- [ ] All interactive elements have a visible `focus-visible` ring.
- [ ] `prefers-reduced-motion` is respected.
- [ ] Semantic HTML is used: button, not div; proper heading hierarchy.
- [ ] All inputs have labels; `aria-label` exists when the visual label is missing.
- [ ] Contrast ratios pass: 4.5:1 for normal text, 3:1 for large text and user interface elements.
- [ ] All sizes are in `rem`.
- [ ] Every `:hover` Rule is gated with `@media (hover: hover) and (pointer: fine)`.

## 6. Verify User experience patterns

### Check that the User always knows what happened and what to do
- [ ] Forms are single column, labels sit above inputs, and validation runs on blur.
- [ ] Error messages appear near the source and include a recovery action.
- [ ] Loading states are present so the User never wonders whether something is happening.
- [ ] Empty states explain the space and provide an action.
- [ ] Destructive actions require confirmation, with the safe action as the primary button.
- [ ] Non-destructive actions are undoable with a toast Undo, not confirmed.
- [ ] Modals trap focus, close on Escape, return focus on close, and prevent body scroll.

## 7. Reject anti-patterns

### Check that generic AI Slop did not enter the surface
- [ ] No gaudy, high-saturation, or rainbow gradients.
- [ ] No excessive rounded corners on everything.
- [ ] No purple-blue-pink color scheme without purpose.
- [ ] No animations that do not serve function.
- [ ] No one-sided colored borders on the left, right, top, or bottom of an element.
- [ ] No divider lines between content sections; spacing, size, and weight create separation except inside accordions or expandable elements.

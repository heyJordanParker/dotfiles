# Interactions

## Timing

- **Hover:** 200ms — Perceptible but not sluggish
- **Active/Press:** Instant (0-50ms) — Immediate feedback
- **Focus:** 150ms — Quick but visible
- **Modal open/close:** 300ms — Deliberate, noticeable
- **Page transition:** 200ms — Smooth but efficient
- **Toast appear:** 200ms — Quick entrance
- **Toast dismiss:** 150ms — Faster exit

**Duration bands by category:**
- **Micro-interactions** (button press, icon swap, hover): 100-150ms
- **Standard UI** (tooltips, dropdowns, focus, popovers): 150-250ms
- **Modals, drawers, large surfaces**: 200-300ms

UI animations stay under 300ms.

**Scale duration with size.** Bigger elements travel farther and read slower. Small elements (buttons, toasts, icons ≤ 20rem) use base durations. Medium containers (panels, cards 20–30rem) × 1.2. Large surfaces (drawers, full-screen sheets ≥ 30rem) × 1.3–1.5.

**Match duration to travel distance.** A 4px nudge and a 400px slide both at 200ms feel wrong — the nudge looks slow, the slide looks frantic. Longer travel earns longer duration; short travel takes the base.

**Rule:** Exits roughly half the duration of entrances (e.g., 150ms exit vs 300ms enter). Exit animations should also be more subtle — use fixed small movement (`-12px`) instead of full element height (`calc(-100% - 4px)`). Exiting elements don't need the same attention as entering ones; full-height exit movement is jarring and competes with incoming content. Never remove exit animation entirely — keep some motion to indicate direction.

## Easing

```css
--ease: cubic-bezier(0.4, 0, 0.2, 1);  /* Material standard */
```

Use `var(--ease)` as the default for most transitions. Starts fast, ends smooth.

**Specialize by intent:**
- **Entering or exiting** → `ease-out` (elements appearing or leaving)
- **On-screen element moving** → `ease-in-out` (position change, size change, any in-place motion)
- **Hover or color transition** → `ease` (native CSS default, right for small state changes)
- **Drag or interruptible gesture** → spring (physics-based; see Interruptibility)
- **Continuous animation** → `linear` (spinners, progress bars)

## States

Every interactive element needs:

- **Default:** Base appearance
- **Hover:** Subtle bg change, slight lift
- **Focus:** Visible ring (accessibility)
- **Active:** Pressed/depressed feel
- **Disabled:** Reduced opacity (50-60%), no pointer
- **Loading:** Spinner or pulse, disabled interaction

**Hover example** — gate `:hover` with `@media (hover: hover) and (pointer: fine)` so styles don't stick on tap on touch devices. See [interactable.md](./interactable.md) → Hover States.

```css
.button {
  @apply transition-[background-color,transform] duration-200;

  &:active {
    @apply bg-primary/80 translate-y-0;
  }

  @media (hover: hover) and (pointer: fine) {
    &:hover {
      @apply bg-primary/90 -translate-y-px;
    }
  }
}
```

**Hover flicker** — when a parent's `:hover` triggers a transform on the parent itself, the parent can re-enter and re-exit hover as it moves under the cursor. Animate a child element instead, leaving the parent's hit area stable.

## Interruptibility

Users change intent mid-interaction (open a dropdown, then immediately want to do something else). Non-interruptible animations make the interface feel broken — it ignores the user's new intent.

- **CSS transitions** interpolate toward the latest state and can be interrupted mid-flight. Use for interactions (dropdowns, toggles, hover states)
- **Keyframe animations** run on a fixed timeline and don't retarget after starting. Use for one-shot sequences (page load, staged reveals)

```css
/* Wrong — keyframe on interactive element, can't interrupt */
.dropdown[data-open="true"] {
  animation: slide-down 300ms ease forwards;
}

/* Right — transition retargets when toggled mid-animation */
.dropdown {
  transition: opacity 300ms ease, transform 300ms ease;
  opacity: 0;
  transform: translateY(-8px);
}
.dropdown[data-open="true"] {
  opacity: 1;
  transform: translateY(0);
}
```

**Springs for drag and interruptible gestures.** Timed curves look mechanical on direct manipulation — dragging a card, swiping a sheet, pulling to refresh. A 300ms transition on a drag release ignores the user's release velocity. Springs model physical momentum: velocity at release carries through to the resting state.

**With Framer Motion:**

```tsx
<motion.div
  drag
  dragConstraints={{ left: 0, right: 0 }}
  dragElastic={0.2}
  transition={{ type: "spring", stiffness: 400, damping: 30 }}
/>
```

**Without Framer Motion:** CSS has no native spring. Use `linear()` easing with spring-like stops (Chrome 113+), or reach for Motion One / a dedicated gesture library. Don't fake it with a long cubic-bezier — it can't respond to release velocity.

## Keyframe Patterns

```css
@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes slide-fade-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes scale-in {
  from {
    opacity: 0;
    transform: scale(0.95);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
```

These are primitives. For state-change recipes built on them (notification badges, panel reveals, dropdowns, modals, text content swaps, in-component page swaps, animated digit reveals), see [transitions.md](transitions.md).

## View Transitions

Use the View Transitions API for visual state changes between views or significant UI updates. Universal browser support since 2025. Never use JavaScript-driven or CSS-class-based page transition patterns.

**Multi-page apps — opt in globally:**

```css
@view-transition {
  navigation: auto;
}
```

**Single-page apps — wrap DOM updates:**

```js
document.startViewTransition(() => {
  updateDOM();
});
```

**Customizing the transition:**

```css
::view-transition-old(root) {
  animation: fade-out 200ms var(--ease);
}

::view-transition-new(root) {
  animation: fade-in 200ms var(--ease);
}
```

**Named transitions for specific elements:**

```css
.hero-image {
  view-transition-name: hero;
}

::view-transition-old(hero) {
  animation: scale-out 300ms var(--ease);
}

::view-transition-new(hero) {
  animation: scale-in 300ms var(--ease);
}
```

- Default to cross-fade — only customize when the default doesn't serve the interaction
- `prefers-reduced-motion` is honored automatically
- Name elements that should animate independently from the page transition

## Accessibility

**Reduced motion:**
```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

**Focus visible:**
```css
.button:focus-visible {
  @apply ring-2 ring-primary ring-offset-2;
}
```

Use `focus-visible` not `focus` - only shows for keyboard navigation.

## Micro-interactions

Small delights that feel intentional:

- **Contextual icon animation** — when icons appear/disappear contextually (copy → check, menu open → close), animate `opacity`, `scale`, and `blur` simultaneously. Without animation, the swap feels abrupt and unresponsive. Use exactly these values — do not deviate:
  - `scale`: `0.25` → `1` (never 0.5 or 0.6)
  - `opacity`: `0` → `1`
  - `filter`: `blur(4px)` → `blur(0px)`

  **With Framer Motion:** Use `AnimatePresence mode="popLayout"` with spring transition. `bounce` must always be `0`:
  ```tsx
  <AnimatePresence mode="popLayout">
    <motion.span
      key={isActive ? "active" : "inactive"}
      initial={{ opacity: 0, scale: 0.25, filter: "blur(4px)" }}
      animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
      exit={{ opacity: 0, scale: 0.25, filter: "blur(4px)" }}
      transition={{ type: "spring", duration: 0.3, bounce: 0 }}
    >
      <Icon />
    </motion.span>
  </AnimatePresence>
  ```

  **Without Framer Motion (CSS cross-fade):** Keep both icons in the DOM — one absolutely positioned over the other. Cross-fade them on state change. Because neither icon unmounts, both enter and exit animate smoothly:
  ```tsx
  <div className="relative">
    <div className={cn(
      "absolute inset-0 flex items-center justify-center",
      "transition-[opacity,filter,scale] duration-300 [transition-timing-function:cubic-bezier(0.2,0,0,1)]",
      isActive ? "scale-100 opacity-100 blur-0" : "scale-[0.25] opacity-0 blur-[4px]"
    )}>
      <ActiveIcon />
    </div>
    <div className={cn(
      "transition-[opacity,filter,scale] duration-300 [transition-timing-function:cubic-bezier(0.2,0,0,1)]",
      isActive ? "scale-[0.25] opacity-0 blur-[4px]" : "scale-100 opacity-100 blur-0"
    )}>
      <InactiveIcon />
    </div>
  </div>
  ```
  The non-absolute icon defines the layout size. Check `package.json` for `motion` or `framer-motion` — if present, use the Motion approach. If not, use CSS cross-fade.

  **When to animate icons:** hover action buttons, state change icons (play→pause, like→liked), contextual toolbars, loading/success indicators. **Don't animate:** static navigation icons, decorative icons, always-visible icons.

- **Icon rotation** on menu expand/collapse
- **Staggered entering elements** — break content into individually animated chunks instead of animating one big block. Three granularity levels:
  - **Sections** (~100ms delay) — title, description, buttons animate separately
  - **Individual elements** (~30-80ms delay between items) — split title into word spans, buttons into individual items
  - Use a `--stagger` CSS variable for clean implementation:

```css
@keyframes enter {
  from { transform: translateY(8px); filter: blur(5px); opacity: 0; }
}
.animate-enter {
  animation: enter 800ms cubic-bezier(0.25, 0.46, 0.45, 0.94) both;
  animation-delay: calc(var(--delay, 0ms) * var(--stagger, 0));
}
```

- **Button press** — `scale(0.96)` on `:active`. Always `0.96`, never smaller than `0.95` (feels exaggerated). Use CSS transitions for interruptibility. Add a `static` prop to disable when motion would be distracting:
  ```css
  .button { @apply transition-transform duration-150 ease-out active:scale-[0.96]; }
  ```
- **Success checkmark** with draw animation
- **Sequential tooltips** — the first tooltip in a hover sequence uses the standard delay and entrance. Subsequent tooltips opened within ~500ms of the previous skip both. Without this, hovering across a row of icons feels gated by the first reveal's pacing.
- **Subtle blur as polish** — when a transition reads as technically correct but still feels off, a subtle blur (under 20px peak) on entry/exit masks the discontinuity the eye is catching. Skip when motion is already physical (large translate, scale change > 0.2) — the blur becomes redundant.
- **Skip animation on page load** — use `initial={false}` on `AnimatePresence` to prevent enter animations on first render. Elements in their default state (toggles, tabs, icon swaps) shouldn't animate in on mount — only on subsequent state changes. Don't use on staggered heroes or intentional entrance animations where the initial animation IS the entrance.

**Don't animate high-frequency interactions.** If users see an interaction 100+ times daily (tab switches, list row toggles, feedback on keystrokes), don't animate it. Animation draws attention; attention is expensive when renewed constantly.

**Keyboard-driven actions specifically — remove animation entirely.** Typing, arrow-key navigation, shortcut activation. The user is moving faster than any animation can keep up with, and animation interferes with the input rhythm.

Don't overdo it. One or two per view maximum.

## Performance

**Never use `transition: all`** or Tailwind's `transition` shorthand (maps to `transition-property: all`). Always specify exact properties. `transition: all` forces the browser to watch every property, causes unexpected transitions on unintended properties, and prevents optimizations.

```css
/* Good */ transition-property: scale, background-color;
/* Bad */  transition: all 150ms ease-out;
```

Tailwind: `transition-transform` covers `transform, translate, scale, rotate`. For multiple non-transform properties: `transition-[scale,opacity,filter]`.

**`will-change` — use sparingly.** Hints the browser to pre-promote an element to a GPU compositing layer, avoiding first-frame stutter. Only for GPU-compositable properties:

- **Use:** `transform`, `opacity`, `filter`, `clip-path`
- **Never:** `will-change: all`, `background-color`, `padding`, `top`, `left`, `width`, `height`

Only add when you observe first-frame stutter (Safari benefits most). Each compositing layer costs memory — don't add preemptively.

**Framer Motion: animate `transform`, not `x`/`y` props.** The `x` and `y` props animate `translate()` via the style attribute, which doesn't hardware-accelerate under load. Use `transform: "translateX(...)"` / `"translateY(...)"` directly to keep the GPU compositing path.

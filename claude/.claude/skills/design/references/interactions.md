# Interactions

## Timing

- **Hover:** 200ms — Perceptible but not sluggish
- **Active/Press:** Instant (0-50ms) — Immediate feedback
- **Focus:** 150ms — Quick but visible
- **Modal open/close:** 300ms — Deliberate, noticeable
- **Page transition:** 200ms — Smooth but efficient
- **Toast appear:** 200ms — Quick entrance
- **Toast dismiss:** 150ms — Faster exit

**Rule:** Exits faster than entrances. Exit animations should also be more subtle — use fixed small movement (`-12px`) instead of full element height (`calc(-100% - 4px)`). Exiting elements don't need the same attention as entering ones; full-height exit movement is jarring and competes with incoming content. Never remove exit animation entirely — keep some motion to indicate direction.

## Easing

```css
--ease: cubic-bezier(0.4, 0, 0.2, 1);  /* Material standard */
```

Use for most transitions. Starts fast, ends smooth.

**Alternatives:**
- `ease-out` for entrances (elements appearing)
- `ease-in` for exits (elements leaving)
- `linear` only for continuous animations (spinners, progress)

## States

Every interactive element needs:

- **Default:** Base appearance
- **Hover:** Subtle bg change, slight lift
- **Focus:** Visible ring (accessibility)
- **Active:** Pressed/depressed feel
- **Disabled:** Reduced opacity (50-60%), no pointer
- **Loading:** Spinner or pulse, disabled interaction

**Hover example:**
```css
.button {
  @apply transition-all duration-200;

  &:hover {
    @apply bg-primary/90 -translate-y-px;
  }

  &:active {
    @apply bg-primary/80 translate-y-0;
  }
}
```

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

- **Contextual icon animation** — when icons appear/disappear contextually (copy → check, menu open → close), animate `opacity`, `scale`, and `blur` simultaneously. Without animation, the swap feels abrupt and unresponsive
- **Icon rotation** on menu expand/collapse
- **Staggered entering elements** — break content into individually animated chunks instead of animating one big block. Three granularity levels:
  - **Sections** (~100ms delay) — title, description, buttons animate separately
  - **Individual elements** (~80ms delay) — split title into word spans, buttons into individual items
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

- **Button press** with slight scale down
- **Success checkmark** with draw animation

Don't overdo it. One or two per view maximum.

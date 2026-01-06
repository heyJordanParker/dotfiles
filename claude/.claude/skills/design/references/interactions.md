# Interactions

## Timing

| Action | Duration | Why |
|--------|----------|-----|
| Hover | 200ms | Perceptible but not sluggish |
| Active/Press | Instant (0-50ms) | Immediate feedback |
| Focus | 150ms | Quick but visible |
| Modal open/close | 300ms | Deliberate, noticeable |
| Page transition | 200ms | Smooth but efficient |
| Toast appear | 200ms | Quick entrance |
| Toast dismiss | 150ms | Faster exit |

**Rule:** Exits faster than entrances.

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

| State | Visual Change |
|-------|---------------|
| Default | Base appearance |
| Hover | Subtle bg change, slight lift |
| Focus | Visible ring (accessibility) |
| Active | Pressed/depressed feel |
| Disabled | Reduced opacity (50-60%), no pointer |
| Loading | Spinner or pulse, disabled interaction |

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

## Page Transitions

```css
.page-enter {
  opacity: 0;
}
.page-enter-active {
  opacity: 1;
  transition: opacity 200ms var(--ease);
}
.page-exit {
  opacity: 1;
}
.page-exit-active {
  opacity: 0.5;
  transition: opacity 200ms var(--ease);
}
```

Exit dims to 50%, enter fades from 0%.

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

- **Icon rotation** on menu expand/collapse
- **Staggered list items** appearing one by one
- **Button press** with slight scale down
- **Success checkmark** with draw animation

Don't overdo it. One or two per view maximum.

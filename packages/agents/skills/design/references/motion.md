# Interactions

- Motion helps the User see a state change.
- CSS transitions are the default for interactive motion because they retarget when state changes.

## 1. Choose duration by interaction size

### Keep ordinary user interface animation under 300ms
Use 100-150ms for micro-interactions such as button press, icon swap, and hover; 150-250ms for tooltips, dropdowns, focus, and popovers; 200-300ms for modals, drawers, and large surfaces.
Example: hover 200ms, active press 0-50ms, focus 150ms, modal open or close 300ms, page transition 200ms, toast appear 200ms, toast dismiss 150ms.

### Scale duration with size and travel
Small elements up to 20rem use base durations. Medium containers from 20rem to 30rem use 1.2 times base. Large surfaces at 30rem or more use 1.3 to 1.5 times base. Longer travel earns longer duration; a 4px nudge and a 400px slide should not share timing.

### Make exits shorter than entrances
Exits are roughly half the duration of entrances and use subtler movement, such as `-12px` instead of full element height. Keep some exit motion to show direction.
Never: remove exit animation entirely or move the exiting element its full height by default.

## 2. Choose easing by intent

### Use the default ease for most transitions
Define `--ease: cubic-bezier(0.4, 0, 0.2, 1)` and use it by default.
Example: `transition: opacity 200ms var(--ease);`.

### Specialize easing by movement type
Use `ease-out` for entering and exiting, `ease-in-out` for on-screen movement, native `ease` for hover or color, spring motion for drag or interruptible gestures, and `linear` for continuous animation such as spinners or progress bars.
Never: a long cubic-bezier pretending to be a spring on direct manipulation.

## 3. Give every interactive element complete states

### Define every state explicitly
Interactive elements need default, hover, focus, active, disabled, and loading states. Disabled state uses 50-60 percent opacity and no pointer. Loading shows a spinner or pulse and disables interaction.

### Keep the hit area stable during hover
When a parent hover transforms the parent itself, hover can flicker as the element moves under the cursor. Animate a child element instead and leave the parent hit area stable.
Never: transform the parent hit area in a way that causes hover re-entry.

## 4. Use interruptible motion for interactions

### Prefer transitions for stateful interaction
CSS transitions interpolate toward the latest state and can be interrupted. Use them for dropdowns, toggles, hover states, and ordinary reveals.
Example:
  ```css
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
Never: keyframes on an interactive dropdown that cannot retarget after the User changes intent.

### Use keyframes for one-shot sequences
Keyframes run on a fixed timeline. Use them for page load, staged reveals, and other one-shot sequences.
Example:
  ```css
  @keyframes slide-fade-in {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }
  ```

### Use spring motion for drag and gestures
Drag, swipe, and pull interactions need release velocity. If Framer Motion is installed, use spring transition with `stiffness: 400` and `damping: 30`. Without Framer Motion, use CSS `linear()` spring-like stops where supported or Motion One or a dedicated gesture library.
Example:
  ```tsx
  <motion.div
    drag
    dragConstraints={{ left: 0, right: 0 }}
    dragElastic={0.2}
    transition={{ type: "spring", stiffness: 400, damping: 30 }}
  />
  ```
Never: a timed 300ms transition for drag release.

## 5. Use View Transitions for page-level changes

### Prefer the View Transitions API for routes and page state
The View Transitions API has universal browser support since 2025 and should handle visual state changes between views or significant page updates.
Example:
  ```css
  @view-transition {
    navigation: auto;
  }
  ```
Example:
  ```js
  document.startViewTransition(() => {
    updateDOM();
  });
  ```
Never: JavaScript-driven or CSS-class-based page transition systems for page-level navigation.

### Customize only when the default does not serve the interaction
Default to cross-fade. Name elements that should animate independently. `prefers-reduced-motion` is honored automatically.
Example:
  ```css
  .hero-image { view-transition-name: hero; }
  ::view-transition-old(hero) { animation: scale-out 300ms var(--ease); }
  ::view-transition-new(hero) { animation: scale-in 300ms var(--ease); }
  ```

## 6. Respect accessibility and reduce motion

### Honor reduced motion globally
When the User requests reduced motion, collapse animation and transition duration.
Example:
  ```css
  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after {
      animation-duration: 0.01ms !important;
      transition-duration: 0.01ms !important;
    }
  }
  ```

### Use `focus-visible`, not `focus`, for rings
Keyboard navigation needs a visible ring without showing focus decoration on every pointer click.
Example:
  ```css
  .button:focus-visible {
    @apply ring-2 ring-primary ring-offset-2;
  }
  ```

## 7. Add micro-interactions sparingly

### Animate contextual icon swaps with fixed values
When icons change contextually, such as copy to check or menu to close, animate opacity, scale, and blur together. Use `scale(0.25)` to `1`, opacity `0` to `1`, and `blur(4px)` to `0px`.
Example: use Framer Motion `AnimatePresence mode="popLayout"` with spring duration `0.3` and `bounce: 0`, or keep both icons in the Document Object Model and CSS cross-fade them.
Never: animate static navigation icons, decorative icons, or always-visible icons.

### Use button press and staggered entrance intentionally
Button press uses `scale(0.96)` on `:active`, never below `0.95`. Staggered entrances can split sections by about 100ms or individual elements by 30-80ms with a `--stagger` variable.
Example:
  ```css
  @keyframes enter {
    from { transform: translateY(8px); filter: blur(5px); opacity: 0; }
  }
  .animate-enter {
    animation: enter 800ms cubic-bezier(0.25, 0.46, 0.45, 0.94) both;
    animation-delay: calc(var(--delay, 0ms) * var(--stagger, 0));
  }
  ```

### Remove motion from high-frequency and keyboard-driven actions
Interactions seen 100 or more times daily should not animate. Typing, arrow-key navigation, and shortcut activation remove animation entirely because the User is moving faster than animation can keep up.
Never: animate tab switches, row toggles, or keystroke feedback just because motion is available.

## 8. Protect performance

### Transition only specific properties
`transition: all` makes the browser watch every property and causes unintended transitions. Tailwind's bare `transition` maps to all properties; use property-specific utilities instead.
Example: `transition-property: scale, background-color;` or `transition-[scale,opacity,filter]`.
Never: `transition: all 150ms ease-out;`.

### Use `will-change` only after observed stutter
`will-change` pre-promotes a layer and costs memory. Use it only for `transform`, `opacity`, `filter`, or `clip-path` when first-frame stutter is observed.
Never: `will-change: all`, `background-color`, `padding`, `top`, `left`, `width`, or `height`.

### Keep Framer Motion on the composited path
Under load, Framer Motion `x` and `y` props can miss the hardware-accelerated path. Animate `transform: "translateX(...)"` or `transform: "translateY(...)"` directly.

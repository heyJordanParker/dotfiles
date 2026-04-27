# Transitions

State-change recipes that correct specific defaults the agent gets wrong: shapes the agent would otherwise reach for JavaScript to do, taste constants the agent doesn't have, and modern CSS patterns that replace older approaches.

Each section is the rule (when + why), the contract (data-attribute + var schema), and the minimal CSS expressing the unique mechanism. Boilerplate (`will-change`, `prefers-reduced-motion`, `transition-property` for non-mechanism properties, asymmetric open/close timing rationale) lives in [motion.md](motion.md) — apply globally.

For icon swaps, see [motion.md](motion.md) → Micro-interactions → Contextual icon animation. Not duplicated here.

## Animate intrinsic dimensions, not transforms

For width or height changes, animate the actual `width` and `height` properties, not `transform: scale()`. Scale distorts content (text, borders, padding all scale with it). Old-tech reach: `ResizeObserver` + RAF tween. Modern: one CSS transition on the intrinsic property.

```css
.resizable {
  transition:
    width  var(--resize-duration) var(--resize-ease),
    height var(--resize-duration) var(--resize-ease);
}
```

State change: set new `width` / `height` directly, or via a state class (`.resizable--compact`). Browser tweens.

## CSS-only digit stagger

For animated number reveals (counters, prices, score changes), stagger digits via a `data-stagger="n"` attribute on each digit — not a JS scheduler, not a counter library.

The contract:
- `.digit-group` wraps digits; gets `.is-animating` to trigger the run.
- Each `.digit` inside picks up the same keyframe.
- `data-stagger="1"`, `"2"`, … delays each digit by `n * var(--digit-stagger)`.
- Direction is a unit-less vector (`--digit-direction-x`, `--digit-direction-y`) multiplied by `--digit-distance`. Flip a sign to change direction without rewriting the keyframe.

```html
<span class="digit-group is-animating">
  <span class="digit">1</span>
  <span class="digit">2</span>
  <span class="digit" data-stagger="1">.</span>
  <span class="digit" data-stagger="2">3</span>
</span>
```

```css
@keyframes digit-pop-in {
  from {
    transform: translate(
      calc(var(--digit-distance) * var(--digit-direction-x)),
      calc(var(--digit-distance) * var(--digit-direction-y))
    );
    opacity: 0;
    filter: blur(var(--digit-blur));
  }
}

.digit-group { display: inline-flex; align-items: baseline; }
.digit { display: inline-block; }

.digit-group.is-animating .digit {
  animation: digit-pop-in var(--digit-duration) var(--digit-ease) both;
}
.digit-group.is-animating .digit[data-stagger="1"] {
  animation-delay: var(--digit-stagger);
}
.digit-group.is-animating .digit[data-stagger="2"] {
  animation-delay: calc(var(--digit-stagger) * 2);
}
```

Replay: remove `.is-animating`, swap text, force reflow (`element.offsetWidth`), re-add `.is-animating`.

## Two-track motion for slide-in-then-pop reveals

When a small element appears with two distinct motions — a wrapper slides in from offset, an inner element pops with scale + blur — use both keyframe and transition on the same element pair. Not one or the other.

- **Outer wrapper:** `animation` (one-shot keyframe) for the slide. Runs once per state toggle. Direction-aware via offset vars.
- **Inner element:** `transition` (interruptible) for the pop. Toggles by `[data-open="true|false"]` on the wrapper.

This honors the motion.md interruptibility rule (transitions for interactive, keyframes for one-shot) at the *element* level — the wrapper's slide is one-shot per show; the inner pop responds to interruption mid-flight if state flips again.

```html
<button class="trigger" style="position: relative">
  <!-- icon -->
  <span class="notification-badge" data-open="true">
    <span class="notification-badge__dot">1</span>
  </span>
</button>
```

```css
@keyframes badge-slide-in {
  from { transform: translate(var(--badge-offset-x), var(--badge-offset-y)); }
  to   { transform: translate(0, 0); }
}

.notification-badge {
  position: absolute;
  top: -0.375rem;
  right: -0.5rem;
  pointer-events: none;
}
.notification-badge[data-open="true"] {
  animation: badge-slide-in var(--badge-slide-duration) var(--badge-slide-ease);
}

.notification-badge__dot {
  display: block;
  transform-origin: center;
  transform: scale(1);
  opacity: 1;
  filter: blur(0);
  transition:
    transform var(--badge-pop-duration)  var(--badge-pop-ease),
    opacity   var(--badge-fade-duration) var(--badge-pop-ease),
    filter    var(--badge-pop-duration)  var(--badge-pop-ease);
}
.notification-badge[data-open="false"] .notification-badge__dot {
  transform: scale(0);
  opacity: 0;
  filter: blur(var(--badge-blur));
  transition:
    transform var(--badge-pop-close-duration)  var(--badge-close-ease),
    opacity   var(--badge-fade-close-duration) var(--badge-close-ease),
    filter    var(--badge-pop-close-duration)  var(--badge-close-ease);
}
```

## Three-phase reflow for text-content swaps

When text content changes (status messages, dynamic labels, processing → complete), don't reach for `<motion.span key={text}>` to force a remount. Drive the swap with two state classes + a forced reflow — no React dependency, no AnimatePresence overhead.

The three phases:
1. JS adds `.is-exit` → old text translates up, blurs, fades out (default transition runs).
2. After `--text-swap-duration`: JS sets new `textContent`, then adds `.is-enter-start` (which jumps the new text *below*, with `transition: none`).
3. JS forces reflow, then removes `.is-enter-start` → new text animates back to 0 with the default transition.

```css
.text-swap {
  display: inline-block;
  transform: translateY(0);
  filter: blur(0);
  opacity: 1;
  transition:
    transform var(--text-swap-duration) var(--text-swap-ease),
    filter    var(--text-swap-duration) var(--text-swap-ease),
    opacity   var(--text-swap-duration) var(--text-swap-ease);
}
.text-swap.is-exit {
  transform: translateY(calc(var(--text-swap-translate-y) * -1));
  filter: blur(var(--text-swap-blur));
  opacity: 0;
}
.text-swap.is-enter-start {
  transform: translateY(var(--text-swap-translate-y));
  filter: blur(var(--text-swap-blur));
  opacity: 0;
  transition: none;
}
```

```js
function swapText(el, next, durationMs) {
  el.classList.add('is-exit');
  setTimeout(() => {
    el.textContent = next;
    el.classList.remove('is-exit');
    el.classList.add('is-enter-start');
    void el.offsetWidth; // force reflow
    el.classList.remove('is-enter-start');
  }, durationMs);
}
```

## Origin-anchored scale + asymmetric open/close

For scale-based show/hide (dropdowns, menus, modals, popovers), two taste rules the agent doesn't apply by default:

**1. `transform-origin` is anchored to where the element visually emerges from**, not center. A dropdown opening below a top-right trigger scales from `top right` — scaling from center makes it look like it comes from the wrong place. Drive via `data-origin` (6 corners). For modals (no anchor), `transform-origin: center` is correct.

**2. Open and close are not mirrors.** Use two state classes — `.is-open` and `.is-closing` — with different durations and different target scales. Open scales *up* from a pre-scale below 1 (e.g., `0.96`). Close scales *outward past 1* (e.g., `1.02`) and fades. Mirroring enter/exit feels mechanical; asymmetry feels intentional.

```css
.dropdown {
  transform-origin: top left;
  transform: scale(var(--menu-pre-scale));
  opacity: 0;
  pointer-events: none;
  transition:
    transform var(--menu-open-duration) var(--menu-ease),
    opacity   var(--menu-open-duration) var(--menu-ease);
}
.dropdown[data-origin="top-right"]     { transform-origin: top right; }
.dropdown[data-origin="top-center"]    { transform-origin: top center; }
.dropdown[data-origin="bottom-left"]   { transform-origin: bottom left; }
.dropdown[data-origin="bottom-center"] { transform-origin: bottom center; }
.dropdown[data-origin="bottom-right"]  { transform-origin: bottom right; }

.dropdown.is-open {
  transform: scale(1);
  opacity: 1;
  pointer-events: auto;
}
.dropdown.is-closing {
  transform: scale(var(--menu-closing-scale)); /* > 1 — outward */
  opacity: 0;
  pointer-events: none;
  transition:
    transform var(--menu-close-duration) var(--menu-ease),
    opacity   var(--menu-close-duration) var(--menu-ease);
}
```

JS contract: add `.is-open` to show. To close, swap `.is-open` for `.is-closing`, then remove `.is-closing` after `--menu-close-duration`.

For modals, drop the `data-origin` rules and use the same `.is-open` / `.is-closing` structure with `transform-origin: center`. Same lesson, different anchor.

## Blur sells short reveals

When a panel reveals with travel less than 100% of its dimension (a sidebar slides 50% of its width, a notification slides 30% of its height), opacity + transform alone read as incomplete — the eye notices the panel hasn't traveled "far enough." Cross-blurring `--panel-blur` ↔ 0 covers the gap and makes a 50% slide read as a full open.

Apply when travel distance is intentionally short. Skip when the panel slides its full dimension — the blur becomes redundant and adds CPU cost.

```css
.panel {
  transform: translateY(var(--panel-translate-y));
  opacity: 0;
  filter: blur(var(--panel-blur));
  pointer-events: none;
  transition:
    transform var(--panel-close-duration) var(--panel-ease),
    opacity   var(--panel-close-duration) var(--panel-ease),
    filter    var(--panel-close-duration) var(--panel-ease);
}
.panel[data-open="true"] {
  transform: translateY(0);
  opacity: 1;
  filter: blur(0);
  pointer-events: auto;
  transition:
    transform var(--panel-open-duration) var(--panel-ease),
    opacity   var(--panel-open-duration) var(--panel-ease),
    filter    var(--panel-open-duration) var(--panel-ease);
}
```

Wrap in a container with `overflow: hidden` if the closed state should be fully clipped. Set `--panel-translate-y` to the travel distance (typically half the panel's own height).

## In-component page swaps don't belong to View Transitions

`motion.md` mandates the View Transitions API for page-level navigation. That mandate does not extend to *in-component* page swaps — wizards inside a card, multi-step forms inside a modal, paginated content inside a panel. View Transitions operate on the document; trying to scope them to a sub-tree fights the API.

For in-component swaps, use absolutely-positioned siblings with directional `data-page-id` and an `--page-exit-enabled` toggle (`0` or `1`) that disables the outgoing slide independently of the incoming.

```html
<div class="page-slider" data-page="1">
  <section class="page" data-page-id="1">…</section>
  <section class="page" data-page-id="2">…</section>
</div>
```

```css
.page-slider { position: relative; }

.page-slider .page[data-page-id="1"] { --page-from-x: calc(var(--page-slide-distance) * -1); }
.page-slider .page[data-page-id="2"] { --page-from-x: var(--page-slide-distance); }

.page-slider .page {
  position: absolute;
  inset: 0;
  opacity: 0;
  pointer-events: none;
  transform: translateX(calc(var(--page-from-x, 0px) * var(--page-exit-enabled)));
  filter: blur(calc(var(--page-blur) * var(--page-exit-enabled)));
  transition:
    opacity   var(--page-fade-duration)  var(--page-fade-ease),
    transform var(--page-slide-duration) var(--page-slide-ease),
    filter    var(--page-slide-duration) var(--page-slide-ease);
}
.page-slider[data-page="1"] .page[data-page-id="1"],
.page-slider[data-page="2"] .page[data-page-id="2"] {
  opacity: 1;
  pointer-events: auto;
  transform: translateX(0);
  filter: blur(0);
  transition-delay: var(--page-stagger);
}
```

JS contract: set `data-page="1"` or `"2"` on `.page-slider`. Set `--page-exit-enabled: 0` to disable the outgoing slide (incoming-only animation, useful when outgoing content shouldn't draw the eye). For >2 pages, extend `[data-page-id="N"]` and matching `[data-page="N"]` selectors — pattern stays the same.

# Transitions

- State-change recipes replace older JavaScript-heavy patterns with CSS-first mechanisms.
- Shared timing, `will-change`, reduced motion, transition-property, and open/close timing Rules live in `motion.md`.
- Icon swap Rules live in `motion.md` and are not duplicated here.

## 1. Animate intrinsic dimensions

### Resize width and height directly
For width or height changes, animate the actual `width` and `height` properties. Modern CSS can tween the intrinsic property without `ResizeObserver` plus `requestAnimationFrame` tweening.
Example:
  ```css
  .resizable {
    transition:
      width  var(--resize-duration) var(--resize-ease),
      height var(--resize-duration) var(--resize-ease);
  }
  ```
Never: `transform: scale()` for resize; it distorts text, borders, and padding.

## 2. Stagger digits with CSS

### Use data attributes instead of a JavaScript scheduler
For animated number reveals, wrap digits in `.digit-group`, toggle `.is-animating`, and give each delayed digit `data-stagger="n"`. Direction is a unitless vector multiplied by distance, so signs can flip without rewriting the keyframe.
Example:
  ```html
  <span class="digit-group is-animating">
    <span class="digit">1</span>
    <span class="digit">2</span>
    <span class="digit" data-stagger="1">.</span>
    <span class="digit" data-stagger="2">3</span>
  </span>
  ```
Example:
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
Never: a counter library for a simple digit reveal.

### Replay by resetting the animation class
Remove `.is-animating`, swap text, force reflow with `element.offsetWidth`, then re-add `.is-animating`.

## 3. Use two tracks for slide-in-then-pop reveals

### Split wrapper slide from inner pop
When a small element appears with two motions, the wrapper uses a one-shot keyframe for slide and the inner element uses an interruptible transition for pop. This keeps one-shot and interactive motion on the correct mechanism.
Example:
  ```html
  <button class="trigger" style="position: relative">
    <span class="notification-badge" data-open="true">
      <span class="notification-badge__dot">1</span>
    </span>
  </button>
  ```
Example:
  ```css
  @keyframes badge-slide-in {
    from { transform: translate(var(--badge-offset-x), var(--badge-offset-y)); }
    to   { transform: translate(0, 0); }
  }
  .notification-badge[data-open="true"] {
    animation: badge-slide-in var(--badge-slide-duration) var(--badge-slide-ease);
  }
  .notification-badge__dot {
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
  }
  ```
Never: put both motions on one keyframe when the inner state can be interrupted.

## 4. Swap text with two state classes and reflow

### Avoid remount-driven text animation
When text content changes, drive the swap with `.is-exit`, `.is-enter-start`, and a forced reflow. This avoids a React remount dependency and AnimatePresence overhead.
Example:
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
Example:
  ```js
  function swapText(el, next, durationMs) {
    el.classList.add('is-exit');
    setTimeout(() => {
      el.textContent = next;
      el.classList.remove('is-exit');
      el.classList.add('is-enter-start');
      void el.offsetWidth;
      el.classList.remove('is-enter-start');
    }, durationMs);
  }
  ```
Never: `<motion.span key={text}>` solely to force remount.

## 5. Anchor scale to origin and make open and close asymmetric

### Anchor transform origin to where the element emerges
Dropdowns opening below a top-right trigger scale from `top right`. Drive six corner origins with `data-origin`. Modals have no anchor, so `transform-origin: center` is correct.
Never: scale every menu from center.

### Make close different from open
Open scales up from below 1, such as `0.96`. Close scales outward past 1, such as `1.02`, and fades. Mirrored enter and exit feels mechanical.
Example:
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
  .dropdown[data-origin="top-right"] { transform-origin: top right; }
  .dropdown.is-open {
    transform: scale(1);
    opacity: 1;
    pointer-events: auto;
  }
  .dropdown.is-closing {
    transform: scale(var(--menu-closing-scale));
    opacity: 0;
    pointer-events: none;
  }
  ```

### Close through an explicit closing state
To close, swap `.is-open` for `.is-closing`, then remove `.is-closing` after `--menu-close-duration`.
Never: instantly remove the element before the close motion can communicate state.

## 6. Use blur to sell short reveals

### Add blur when travel is intentionally short
When a panel travels less than 100 percent of its dimension, opacity plus transform can read as incomplete. Cross-blur from `--panel-blur` to `0` so a 50 percent slide reads as a full open.
Example:
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
  }
  ```
Never: add blur when the panel already slides its full dimension.

### Clip closed panels when needed
Wrap the panel in an `overflow: hidden` container if the closed state should be fully clipped. Set `--panel-translate-y` to the travel distance, often half the panel's own height.

## 7. Use local swaps for in-component pages

### Do not use View Transitions inside a component
The View Transitions API is for page-level navigation and document-level transitions. Wizards inside cards, multi-step forms inside modals, and paginated content inside panels use local absolutely positioned siblings.
Never: fight the document-level API to scope it to a sub-tree.

### Use directional page identifiers and an exit toggle
Use `data-page-id` on each page and `data-page` on the wrapper. An `--page-exit-enabled` value of `0` or `1` lets the outgoing slide disable independently from the incoming slide.
Example:
  ```html
  <div class="page-slider" data-page="1">
    <section class="page" data-page-id="1">…</section>
    <section class="page" data-page-id="2">…</section>
  </div>
  ```
Example:
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

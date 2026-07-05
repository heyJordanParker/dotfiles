# Surfaces

- Surfaces are cards, modals, popovers, dropdowns, panels, sidebars, sticky headers, and any element that must read as separate from the page.
- Light comes from the top. Top edges are lighter; bottom edges are darker.
- Reference cards were traced from kargul.studio's Shadow Breakdown; every tier composes a `0 0 0 1px` ring as layer one.

## 1. Compose elevation from three layers

### Use ring, ambient, and contact layers
The ring is `0 0 0 1px <ring-color>` and replaces a 1px border. Ambient is the large-blur low-opacity shadow that establishes elevation. Contact is the small-blur shadow near the surface that keeps the ambient layer attached.
Never: a single invented shadow for an elevated surface.

### Keep one shadow color and scale only opacity, offset, and blur
Light mode ring is `#E5E7EB`; ambient and contact use `#030712`. The tiers change opacity, offset, and blur only.

## 2. Define the tier tokens

### Put the shared tokens in `theme.css`
The tier tokens are the source of truth for surface elevation and are bridged to Tailwind in `index.css`.
Template:
  ```css
  /* theme.css */
  --shadow-ring: oklch(0.92 0.005 250);
  --shadow-color: 3 7 18;

  --shadow-subtle:
    0 0 0 1px var(--shadow-ring),
    0 2px 3px 0 rgb(var(--shadow-color) / 0.20);

  --shadow-elevated:
    0 0 0 1px var(--shadow-ring),
    2px 4px 24px 0 rgb(var(--shadow-color) / 0.20),
    0 2px 6px 0 rgb(var(--shadow-color) / 0.16);

  --shadow-floating:
    0 0 0 1px var(--shadow-ring),
    8px 6px 36px 0 rgb(var(--shadow-color) / 0.28),
    0 4px 16px 0 rgb(var(--shadow-color) / 0.12);

  --shadow-extruded:
    0 0 0 1px var(--shadow-ring),
    5px 5px 10px 0 rgb(var(--shadow-color) / 0.24),
    -5px -5px 10px 0 rgb(var(--shadow-color) / 0.12);
  ```

Template:
  ```css
  @theme inline {
    --shadow-subtle:   var(--shadow-subtle);
    --shadow-elevated: var(--shadow-elevated);
    --shadow-floating: var(--shadow-floating);
    --shadow-extruded: var(--shadow-extruded);
  }
  ```

## 3. Pick the closest tier

### Use subtle for resting surfaces
Subtle is ring plus a tight contact shadow. Use it for resting cards, sidebar bodies, and list rows.
Example: ring `0 0 0 1 #E5E7EB 100%`; contact `0 2 3 0 #030712 20%`.

### Use elevated for hover and attached overlays
Elevated is ring plus diffuse ambient plus small contact. Use it for hover, dropdowns, popovers, and sticky headers.
Example: ambient `2 4 24 0 #030712 20%`; contact `0 2 6 0 #030712 16%`.

### Use floating for detached overlays
Floating is ring plus heavy ambient plus medium contact. Use it for modals, command menus, and toast stacks; the 28 percent peak opacity reads over arbitrary content.
Example: ambient `8 6 36 0 #030712 28%`; contact `0 4 16 0 #030712 12%`.

### Use extruded only for specialized neumorphic surfaces
Extruded uses equal blur and opposite offsets: `5 5` cast shadow and `-5 -5` highlight. It competes for attention against standard tiers, so use it sparingly.
Never: extruded as the default card style.

### Stay on the elevation curve
Subtle has contact only. Elevated and floating add ambient: Y offset grows from 4 to 6, blur from 24 to 36, X offset from 2 to 8, and peak opacity from 20 percent to 28 percent.
Never: invent intermediate values; pick the closest tier.

## 4. Replace borders with the ring

### Use the shadow ring for adaptable boundaries
Solid borders do not adapt to gradients, images, or dark backgrounds. The ring is always layer one of every tier token, so applying a tier already provides the boundary.
Example: `box-shadow: 0 0 0 1px var(--shadow-ring);`.
Never: `border: 1px solid #e5e7eb` for a surface boundary.

### Hover moves one tier up
A subtle card can become elevated on hover, using the pointer gate from `interactable.md`.
Example:
  ```css
  .card {
    box-shadow: var(--shadow-subtle);
    transition: box-shadow 200ms var(--ease);
  }
  @media (hover: hover) {
    .card:hover { box-shadow: var(--shadow-elevated); }
  }
  ```

## 5. Build tactile primary buttons as layered surfaces

### Use all six tactile layers
A tactile primary uses base fill, inset edge-light stroke, brand-rim drop shadow, ambient drop shadow, optional contact drop shadow, and top gloss inner shadow. Each layer has one purpose; removing one makes the button flat or mismatched.
Never: tactile recipe on secondary or tertiary buttons.

### Derive rim and gloss from the fill
The rim color comes from the fill, one step darker and full saturation. Gloss stays at the top. The fill may be solid or a two-stop gradient with top lightness at most `0.05` above bottom.
Never: gloss at the bottom or rim picked from a separate brand palette.

Template:
  ```css
  .button--tactile {
    display: inline-flex;
    align-items: center;
    gap: 0.5em;
    padding-block: 0.75lh;
    padding-inline-start: 1em;
    padding-inline-end: 1.25em;
    border-radius: var(--btn-radius, calc(var(--radius) + 0.25rem));
    color: var(--btn-label, #fff);
    background:
      linear-gradient(
        to bottom,
        rgb(255 255 255 / var(--btn-stroke-top, 0.4)) 0,
        rgb(255 255 255 / var(--btn-stroke-bottom, 0)) 1px,
        transparent 1px
      ),
      var(--btn-fill);
    box-shadow:
      inset 0 var(--btn-gloss-y, 2px) var(--btn-gloss-blur, 6px)
        var(--btn-gloss-spread, 0)
        rgb(255 255 255 / var(--btn-gloss, 0.20)),
      0 var(--btn-ambient-y, 3px) var(--btn-ambient-blur, 6px) 0
        rgb(0 0 0 / var(--btn-ambient, 0.20)),
      0 var(--btn-contact-y, 2px) var(--btn-contact-blur, 4px) 0
        rgb(0 0 0 / var(--btn-contact, 0)),
      0 0 0 1px var(--btn-rim, transparent);
    transition: scale 150ms ease-out;
    &:active { scale: 0.96; }
  }
  ```

### Use traced presets as proof of the mechanism
Three preset variable blocks were traced from real designs: blue for software as a service admin, black for premium editorial, and orange for bold playful surfaces. Same recipe, different tokens, so the mechanism is parameterized.
Example: `--btn-fill: #296FF0; --btn-rim: #1B5BD0; --btn-gloss: 0.24;`.
Never: flat single-shadow primaries on hero, landing, or marketing surfaces.

## 6. Handle dark mode and special surfaces

### Collapse dark mode shadows to a white ring
Layered ambient and contact shadows disappear against dark backgrounds, so each tier collapses to a single white ring.
Template:
  ```css
  .dark {
    --shadow-subtle:   0 0 0 1px rgb(255 255 255 / 0.08);
    --shadow-elevated: 0 0 0 1px rgb(255 255 255 / 0.13);
    --shadow-floating: 0 0 0 1px rgb(255 255 255 / 0.18);
  }
  ```

### Recess inputs and pressed states with inset shadows
Inputs, wells, and pressed states use dark inset top plus light inset bottom.
Example:
  ```css
  .input {
    box-shadow:
      inset 0 1px 2px rgb(var(--shadow-color) / 0.10),
      inset 0 -1px 0 rgb(255 255 255 / 0.05);
  }
  ```

### Use floating tier for glass
Glass surfaces combine `backdrop-blur`, semi-transparent background, and the floating tier shadow.
Example:
  ```css
  .glass {
    background: rgb(255 255 255 / 0.6);
    backdrop-filter: blur(12px);
    box-shadow: var(--shadow-floating);
  }
  ```

### Outline images at low opacity
Images can disappear when edge colors match the background. Apply a 1px outline at low opacity and offset it inward.
Example:
  ```css
  .image-outline {
    outline: 1px solid rgb(var(--shadow-color) / 0.10);
    outline-offset: -1px;
  }
  .dark .image-outline {
    outline-color: rgb(255 255 255 / 0.10);
  }
  ```

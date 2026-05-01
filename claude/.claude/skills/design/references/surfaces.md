# Surfaces

How UI surfaces sit, bound, and react. Covers elevation tiers, the ring that replaces solid borders, recessed inputs, glass panels, and image outlines. Use when building cards, modals, popovers, dropdowns, panels, sidebars, sticky headers, and any element that needs to read as separate from the page.

## Three-layer composition

Every elevated surface composes from up to three layers:

1. **Ring** — `0 0 0 1px <ring-color>`, replaces a 1px border. Always present. Adapts to gradients, images, and dark backgrounds via transparency in a way solid borders cannot.
2. **Ambient** — large-blur, low-opacity shadow that establishes elevation. Blur and offset scale with how high the surface sits.
3. **Contact** — small-blur shadow near the surface. Adds definition, prevents the ambient shadow from looking detached.

Single shadow color across tiers — only opacity, offset, and blur change. Light mode ring is `#E5E7EB`; ambient and contact use `#030712`.

## Tokens

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

Bridge to Tailwind in `index.css`:

```css
@theme inline {
  --shadow-subtle:   var(--shadow-subtle);
  --shadow-elevated: var(--shadow-elevated);
  --shadow-floating: var(--shadow-floating);
  --shadow-extruded: var(--shadow-extruded);
}
```

## Tier recipes

Exact values, not approximations. Reference cards traced from kargul.studio's "Shadow Breakdown" — every tier composes a `0 0 0 1px` ring as layer 1.

### Subtle — resting cards, sidebar bodies, list rows

Ring + a single tight contact shadow. Reads as "barely lifted from the page."

| Layer    | X | Y | Blur | Spread | Color    | Opacity |
|----------|---|---|------|--------|----------|---------|
| Ring     | 0 | 0 | 0    | 1      | #E5E7EB  | 100%    |
| Contact  | 0 | 2 | 3    | 0      | #030712  | 20%     |

### Elevated — hover, dropdowns, popovers, sticky headers

Ring + diffuse ambient + small contact. Ambient blur is wide (24px) and offset slightly right (`X: 2`) — places the light source upper-left.

| Layer    | X | Y | Blur | Spread | Color    | Opacity |
|----------|---|---|------|--------|----------|---------|
| Ring     | 0 | 0 | 0    | 1      | #E5E7EB  | 100%    |
| Ambient  | 2 | 4 | 24   | 0      | #030712  | 20%     |
| Contact  | 0 | 2 | 6    | 0      | #030712  | 16%     |

### Floating — modals, command menus, toast stacks

Ring + heavy ambient + medium contact. Largest offsets (`X: 8, Y: 6`) and blur (36px). Peak opacity rises to 28% — high enough to read against arbitrary content beneath the surface.

| Layer    | X | Y | Blur | Spread | Color    | Opacity |
|----------|---|---|------|--------|----------|---------|
| Ring     | 0 | 0 | 0    | 1      | #E5E7EB  | 100%    |
| Ambient  | 8 | 6 | 36   | 0      | #030712  | 28%     |
| Contact  | 0 | 4 | 16   | 0      | #030712  | 12%     |

### Extruded — specialized neumorphic surfaces

Inverse offset pair: equal blur, opposite X/Y, asymmetric opacity. The `(-5,-5)` lighter shadow reads as a top-light highlight; the `(5,5)` heavier shadow reads as the cast shadow. Use sparingly — extruded surfaces compete for attention against the standard tiers.

| Layer    | X  | Y  | Blur | Spread | Color    | Opacity |
|----------|----|----|------|--------|----------|---------|
| Ring     | 0  | 0  | 0    | 1      | #E5E7EB  | 100%    |
| Cast     | 5  | 5  | 10   | 0      | #030712  | 24%     |
| Highlight| -5 | -5 | 10   | 0      | #030712  | 12%     |

## Elevation curve — how the values scale

Subtle is the entry point — a single Contact layer (Y:2, Blur:3, 20%), no Ambient at this tier.

Elevated and Floating add an Ambient layer that grows with elevation:

- **Ambient Y offset:** 4 → 6 (linear)
- **Ambient blur:** 24 → 36 (taper)
- **Ambient X offset:** 2 → 8 (rightward bias grows; places the light source upper-left)
- **Peak opacity:** 20% → 28% (Floating breaks 20% to read against arbitrary content)

Stay on these curves when introducing a new tier. Don't invent intermediate values — pick the closest tier and use it.

## Ring as border replacement

Solid border colors don't adapt to varied backgrounds:

```css
/* Wrong — solid border breaks on non-white backgrounds */
border: 1px solid #e5e7eb;

/* Right — shadow-based ring adapts via transparency */
box-shadow: 0 0 0 1px var(--shadow-ring);
```

Ring is always layer 1 of every tier token — applying `var(--shadow-subtle)` already gives you the ring; no separate border declaration needed.

## Hover transitions one tier up

Gate the hover rule with `@media (hover: hover)` so the elevated shadow doesn't stick on tap on touch devices. See [interactable.md](./interactable.md) → Hover States.

```css
.card {
  box-shadow: var(--shadow-subtle);
  transition: box-shadow 200ms var(--ease);
}
@media (hover: hover) {
  .card:hover { box-shadow: var(--shadow-elevated); }
}
```

## Tactile primaries

Primary CTAs earn a layered tactile stack — multiple shadow layers fake physicality so the button looks pressable. Six layers, every one with a single purpose; remove one and the surface looks flat or mismatched.

Layer order, bottom to top:

1. **Base fill** — solid color, or 2-stop linear gradient with top lighter than bottom (lightness difference ≤ 0.05)
2. **Inset edge-light stroke** — white at 15–40% opacity, applied as an inset linear gradient (top brighter than bottom). Fakes light catching the lip
3. **Brand-rim drop shadow** — drop shadow in a color derived from the fill (one step darker, full saturation) at 100% opacity, spread 1, blur 0. Creates the perimeter ring that reads as the surface's own edge
4. **Ambient drop shadow** — black at 8–20% opacity, blur 2–6, y 1–3. The cast shadow that grounds the surface
5. **Contact drop shadow** — optional second drop, sharper. Black at 15–20% opacity, blur 4, y 2. Use when the surface needs more grounding on bright backgrounds
6. **Top gloss inner shadow** — inset white at 15–24% opacity, blur 4–6, y 1–4. Top-only highlight from the light source above

Light source at the top is mandatory — gloss never goes at the bottom. Rim color is always derived from the fill, never picked from a separate brand palette.

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
    /* top gloss */
    inset 0 var(--btn-gloss-y, 2px) var(--btn-gloss-blur, 6px)
      var(--btn-gloss-spread, 0)
      rgb(255 255 255 / var(--btn-gloss, 0.20)),
    /* ambient */
    0 var(--btn-ambient-y, 3px) var(--btn-ambient-blur, 6px) 0
      rgb(0 0 0 / var(--btn-ambient, 0.20)),
    /* contact */
    0 var(--btn-contact-y, 2px) var(--btn-contact-blur, 4px) 0
      rgb(0 0 0 / var(--btn-contact, 0)),
    /* brand rim */
    0 0 0 1px var(--btn-rim, transparent);
  transition: scale 150ms ease-out;

  &:active { scale: 0.96; }
}
```

### Presets

Three preset variable blocks, traced from real designs. Same recipe, different tokens — proves the mechanism is parameterized.

```css
/* Blue primary — SaaS / admin (e.g. "+ Add team member") */
.button--tactile-blue {
  --btn-fill: #296FF0;
  --btn-rim: #1B5BD0;
  --btn-stroke-top: 0.40;
  --btn-ambient: 0.20;  --btn-ambient-y: 3px;  --btn-ambient-blur: 6px;
  --btn-contact: 0;
  --btn-gloss: 0.24;    --btn-gloss-y: 2px;    --btn-gloss-blur: 6px;
  --btn-gloss-spread: 2px;
}

/* Black primary — premium / editorial (e.g. "Get Started") */
.button--tactile-black {
  --btn-fill: linear-gradient(to bottom, #312F37, #18171C);
  --btn-rim: #18171C;
  --btn-stroke-top: 0.15;
  --btn-ambient: 0.15;  --btn-ambient-y: 2px;  --btn-ambient-blur: 4px;
  --btn-contact: 0;
  --btn-gloss: 0.15;    --btn-gloss-y: 4px;    --btn-gloss-blur: 4px;
  /* Light halo: replace rim with negative-spread light drop */
  box-shadow:
    inset 0 4px 4px rgb(255 255 255 / 0.15),
    0 2px 4px rgb(1 1 1 / 0.15),
    0 4px 4px -3px rgb(180 178 189 / 1);
}

/* Orange primary — bold / playful (e.g. "+ Add view") */
.button--tactile-orange {
  --btn-fill: #FF6C05;
  --btn-rim: #EB6100;
  --btn-stroke-top: 0.26;
  --btn-ambient: 0.08;  --btn-ambient-y: 1px;  --btn-ambient-blur: 2px;
  --btn-contact: 0.16;  --btn-contact-y: 2px;  --btn-contact-blur: 4px;
  --btn-gloss: 0.16;    --btn-gloss-y: 1px;    --btn-gloss-blur: 20px;
}
```

### Anti-patterns

- Tactile recipe on secondary or tertiary buttons — devalues hierarchy
- Gloss anywhere but the top — contradicts top-light-source rule
- Rim color picked from a brand palette instead of derived from the fill — the rim stops reading as the surface's edge and starts reading as a separate ring
- Flat single-shadow primaries on hero, landing, or marketing surfaces — under-delivers affordance
- Border-radius mismatch between outer surface and inner content (icon, badge) — outer must equal inner + padding

## Dark mode

Layered ambient/contact shadows go invisible against dark backgrounds. Collapse to a single white ring per tier:

```css
.dark {
  --shadow-subtle:   0 0 0 1px rgb(255 255 255 / 0.08);
  --shadow-elevated: 0 0 0 1px rgb(255 255 255 / 0.13);
  --shadow-floating: 0 0 0 1px rgb(255 255 255 / 0.18);
}
```

## Recessed elements

Inputs, wells, and pressed states use inset shadows: dark inset top (shadow falling into the recess) + light inset bottom (light hitting the recess floor):

```css
.input {
  box-shadow:
    inset 0 1px 2px rgb(var(--shadow-color) / 0.10),
    inset 0 -1px 0 rgb(255 255 255 / 0.05);
}
```

## Glass surfaces

`backdrop-blur` + semi-transparent background + floating tier shadow:

```css
.glass {
  background: rgb(255 255 255 / 0.6);
  backdrop-filter: blur(12px);
  box-shadow: var(--shadow-floating);
}
```

## Image outlines

Images blend into surrounding content when edge colors match the background. Apply a 1px outline at low opacity:

```css
.image-outline {
  outline: 1px solid rgb(var(--shadow-color) / 0.10);
  outline-offset: -1px;
}
.dark .image-outline {
  outline-color: rgb(255 255 255 / 0.10);
}
```

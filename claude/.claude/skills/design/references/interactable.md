# UX Patterns

## Buttons

Primary CTAs earn a layered tactile stack — multiple shadow layers fake physicality so the button looks pressable. Secondary and tertiary buttons stay flat. Hierarchy reads through stack depth, never through size.

### Hierarchy

- One primary per view — never two competing CTAs
- Primary: tactile stack (recipe below)
- Secondary: flat fill or outline, no stack
- Tertiary: text-only or ghost
- Destructive: primary stack with destructive token; in confirmation dialogs the safe action stays primary (see Confirmation)

### Tactile primary surfaces

Layer order from bottom to top. Every layer has a single purpose; remove one and the button looks flat or mismatched.

1. **Base fill** — solid color, or 2-stop linear gradient with top lighter than bottom (lightness difference ≤ 0.05)
2. **Inset edge-light stroke** — white at 15–40% opacity, applied as an inset linear gradient (top brighter than bottom). Fakes light catching the lip
3. **Brand-rim drop shadow** — drop shadow in a color derived from the fill (one step darker, full saturation) at 100% opacity, spread 1, blur 0. Creates the perimeter ring that reads as the button's own edge
4. **Ambient drop shadow** — black at 8–20% opacity, blur 2–6, y 1–3. The cast shadow that grounds the button
5. **Contact drop shadow** — optional second drop, sharper. Black at 15–20% opacity, blur 4, y 2. Use when the button needs more grounding on bright backgrounds
6. **Top gloss inner shadow** — inset white at 15–24% opacity, blur 4–6, y 1–4. Top-only highlight from the light source above

Light source is at the top — gloss never goes at the bottom (matches Shadows & Elevation rule in Skill.md). Rim color is always derived from the fill, never picked from a separate brand palette.

Pair every primary with `:active scale(0.96)` (motion.md → Micro-interactions) and a `focus-visible` ring (motion.md → Accessibility). Concentric border radius applies (Skill.md → Radius). Icon+label primaries use asymmetric padding, less on the icon side (Skill.md → Optical Corrections).

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

### Examples

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
- Two or more primary buttons in the same view — no one thing to press
- Gloss anywhere but the top — contradicts top-light-source rule
- Rim color picked from a brand palette instead of derived from the fill — the rim stops reading as the button's edge and starts reading as a separate ring
- Flat single-shadow primaries on hero, landing, or marketing surfaces — under-delivers affordance
- Skipping `:active scale(0.96)` — press feedback completes the tactile contract
- Border-radius mismatch between button and inner content (icon, badge) — outer must equal inner + padding

## Forms

**Layout:**
- Single column (don't split fields side-by-side)
- Labels above inputs (not inline or placeholder-only)
- Group related fields with spacing, not boxes

**Validation:**
- Validate on blur, not on every keystroke
- Show errors below the field, not in alerts
- Clear error when user starts fixing

**Submission:**
- Disable button during processing
- Show loading state (spinner in button)
- Success: brief toast + redirect or state change
- Error: specific message, keep form data

## Navigation

**Patterns:**
- Sidebar for main sections (admin)
- Tabs for sub-sections within a page
- Breadcrumbs for deep hierarchies
- Back button for linear flows

**Active states:**
- Clear visual indicator (bg color, border, weight)
- Only one active item per navigation level

## Feedback

**Loading:**
- Skeleton screens for layout-known content
- Spinners for unknown layout
- Never leave user wondering if something is happening

**Success:**
- Brief toast (3-5 seconds, auto-dismiss)
- Or inline confirmation near the action
- Don't redirect without clear signal

**Errors:**
- Specific message ("Email already registered" not "Error")
- Near the source (inline, not page-top banner)
- Recoverable: suggest action ("Try again" or "Use different email")

## Empty States

Don't show blank space. Provide:
- Clear explanation of what belongs here
- Action to add first item
- Optional illustration (not required)

## Confirmation

**Destructive actions** (delete, remove, cancel):
- Require confirmation dialog
- Primary button is the safe action (Cancel)
- Destructive button is secondary/red
- State what will be deleted

**Non-destructive actions:**
- Don't ask for confirmation
- Make it undoable instead (toast with Undo)

## Accessibility

**Keyboard navigation:**
- All interactive elements focusable
- Logical tab order (DOM order)
- Enter/Space to activate
- Escape to close modals/dropdowns

**Screen readers:**
- Semantic HTML (button, not div with click handler)
- Labels on all inputs
- `aria-label` when visual label missing
- `hidden-accessible` class for visually hidden but announced text:
```css
.hidden-accessible {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  border: 0;
}
```

**Minimum hit area:** Interactive elements need at least 44x44px (WCAG) or 40x40px minimum. If the visible element is smaller, extend with a pseudo-element:
```css
.checkbox {
  position: relative;
  width: 20px;
  height: 20px;
}
.checkbox::after {
  content: "";
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 44px;
  height: 44px;
}
```
Tailwind: `relative size-5 after:absolute after:top-1/2 after:left-1/2 after:size-11 after:-translate-1/2`

**Collision rule:** Never let extended hit areas of two interactive elements overlap. Shrink the pseudo-element if needed — but make it as large as possible without colliding.

**Focus indicators:**
- Visible focus ring on all interactive elements
- `focus-visible` for keyboard-only visibility
- Parent focus: `focus-parent` class for container indication

**Clickable containers:**
- If whole card is clickable, use `clickable-parent` pattern:
```css
.clickable-parent {
  position: relative;
}
.clickable-parent a::after {
  content: "";
  position: absolute;
  inset: 0;
}
```

## Modals & Dialogs

- Trap focus inside modal
- Close on Escape
- Close on backdrop click (unless destructive action)
- Return focus to trigger element on close
- Prevent body scroll while open

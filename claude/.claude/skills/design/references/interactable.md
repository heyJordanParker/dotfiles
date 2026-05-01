# UX Patterns

## Buttons

Hierarchy reads through surface depth, never through size. Primary CTAs use the tactile surface stack; secondary and tertiary stay flat.

### Hierarchy

- One primary per view — never two competing CTAs
- Primary: tactile stack — see surfaces.md → Tactile primaries for the layer recipe and presets
- Secondary: flat fill or outline, no stack
- Tertiary: text-only or ghost
- Destructive: primary tactile stack with destructive token; in confirmation dialogs the safe action stays primary (see Confirmation)

### Behavior

Pair every primary with `:active scale(0.96)` (motion.md → Micro-interactions) and a `focus-visible` ring (motion.md → Accessibility). Concentric border radius applies (Skill.md → Radius). Icon+label primaries use asymmetric padding, less on the icon side (Skill.md → Optical Corrections).

### Anti-patterns

- Two or more primary buttons in the same view — no one thing to press
- Skipping `:active scale(0.96)` — press feedback completes the tactile contract

## Hover States

Gate every `:hover` rule with `@media (hover: hover)`. On touch devices, tapping triggers `:hover` styles and they stay stuck until another element is tapped — buttons remain "lit up", cards stay elevated. The media query restricts hover styles to devices with a real pointer.

Apply only to `:hover` — `:active`, `:focus-visible`, and `:focus` work on touch and stay outside the gate.

```css
.button {
  @apply transition-[background-color,transform] duration-200;

  &:active { @apply bg-primary/80 translate-y-0; }

  @media (hover: hover) {
    &:hover { @apply bg-primary/90 -translate-y-px; }
  }
}
```

For surface elevation on hover (card lifts to a higher tier), the same rule applies:

```css
.card {
  box-shadow: var(--shadow-subtle);
  transition: box-shadow 200ms var(--ease);
}
@media (hover: hover) {
  .card:hover { box-shadow: var(--shadow-elevated); }
}
```

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

# UX Patterns

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

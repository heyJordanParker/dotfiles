# User Experience Patterns

- User interface Affordances must make the available action, result, and recovery path visible to the User.
- Surface depth carries button hierarchy; size does not.

## 1. Build buttons with one primary action

### Keep one primary per view
A view gets one primary button. Secondary buttons use flat fill or outline. Tertiary buttons are text-only or ghost. Destructive buttons use the destructive token, but in confirmation dialogs the safe action stays primary.
Never: two competing primary buttons in the same view.

### Give primary buttons tactile feedback
Primary buttons use the tactile surface stack from `surfaces.md`, `:active { scale: 0.96; }`, and a visible `focus-visible` ring. Icon and label primaries use asymmetric padding with less padding on the icon side.
Never: skip press feedback on a tactile primary.

## 2. Gate hover states

### Limit `:hover` to real pointer devices
On touch devices, tapped `:hover` styles can stick. Wrap every hover Rule in `@media (hover: hover) and (pointer: fine)`. Leave `:active`, `:focus-visible`, and `:focus` outside the gate.
Example:
  ```css
  .button {
    @apply transition-[background-color,transform] duration-200;
    &:active { @apply bg-primary/80 translate-y-0; }
    @media (hover: hover) and (pointer: fine) {
      &:hover { @apply bg-primary/90 -translate-y-px; }
    }
  }
  ```

### Lift surfaces one tier on hover
A card at `var(--shadow-subtle)` can lift to `var(--shadow-elevated)` on hover, using the same pointer gate.
Example:
  ```css
  .card {
    box-shadow: var(--shadow-subtle);
    transition: box-shadow 200ms var(--ease);
  }
  @media (hover: hover) and (pointer: fine) {
    .card:hover { box-shadow: var(--shadow-elevated); }
  }
  ```

## 3. Make forms linear and recoverable

### Keep form layout single-column
Labels sit above inputs. Related fields group through spacing, not boxes.
Never: split fields side-by-side by default or rely on placeholders as labels.

### Validate after the User leaves the field
Validate on blur, show the error below the field, and clear the error when the User starts fixing it.
Never: validate on every keystroke or show field errors in page-level alerts.

### Preserve the User's work during submission
Disable the button while processing, show a loading state in the button, show success with a brief toast plus redirect or state change, and keep form data on error.
Never: erase entered data after a recoverable error.

## 4. Make navigation state unambiguous

### Use the navigation pattern that matches the task
Use sidebars for admin main sections, tabs for sub-sections within a page, breadcrumbs for deep hierarchy, and back buttons for linear movement.

### Show one active item per navigation level
Active state can use background, border, or weight, but only one item is active at each level.
Never: show multiple active items in the same navigation level.

## 5. Put feedback near the action

### Use loading states that match known layout
Use skeleton screens when the layout is known and spinners when the layout is unknown. The User should never wonder whether something is happening.

### Make success brief and local
Use a 3 to 5 second toast or inline confirmation near the action. Do not redirect without a clear signal.

### Make errors specific and recoverable
Say the specific problem and recovery action near the source.
Example: "Email already registered. Use a different email."
Never: "Error" as the whole message.

## 6. Fill empty states and confirmations

### Empty states explain the space and the first action
An empty state says what belongs there and gives the action to add the first item. An illustration is optional.
Never: blank space where the User expects content.

### Confirm destructive actions only
Delete, remove, and cancel actions require confirmation. The primary button is the safe action, and the destructive button is secondary or red. State what will be deleted.
Never: make the destructive action the primary button.

### Make non-destructive actions undoable
Non-destructive actions get a toast with Undo instead of a confirmation dialog.
Never: interrupt non-destructive actions with confirmation.

## 7. Preserve accessibility contracts

### Keep keyboard behavior complete
All interactive elements are focusable in logical Document Object Model order. Enter or Space activates. Escape closes modals and dropdowns.

### Use semantic HTML and labels
Use `button`, not `div` with a click handler. Inputs have labels. Use `aria-label` when the visual label is missing. Use `hidden-accessible` for visually hidden announced text.
Example:
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

### Maintain a 44 by 44px hit area
Interactive elements need at least a 44 by 44px hit area under Web Content Accessibility Guidelines, with 40 by 40px as the absolute minimum. Extend smaller visible elements with a pseudo-element.
Example:
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
Never: let extended hit areas overlap between adjacent elements.

### Show focus visibly
Every interactive element has a visible `focus-visible` ring. Use a parent focus class when a container needs the indication.

### Make clickable containers honest
If a whole card is clickable, use the clickable-parent pattern so the link covers the surface without hiding semantics.
Example:
  ```css
  .clickable-parent { position: relative; }
  .clickable-parent a::after {
    content: "";
    position: absolute;
    inset: 0;
  }
  ```

## 8. Make modals behave like modals

### Trap focus and restore it
Modals trap focus, close on Escape, close on backdrop click unless the action is destructive, return focus to the trigger on close, and prevent body scroll while open.
Never: leave keyboard focus behind the modal.

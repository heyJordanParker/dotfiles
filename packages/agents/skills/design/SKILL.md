---
name: design
description: TRIGGER when building UI components, styling pages, or making visual/interaction Decisions. Guides design thinking, anti-patterns, CSS Architecture, and responsive patterns.
---

# Frontend Design

- Good design is as little design as possible: fewer colors, fewer words, less clutter.
- Minimal mode is the default for admin surfaces, utilities, dashboards, and dense product work.
- Bold mode is for landing and marketing surfaces that must be distinctive and memorable.
- Intentionality beats intensity; every visual choice must serve the User.

## 1. Name the purpose and mode

Start with what the surface accomplishes for the User, the tone, the space and existing patterns it must respect, and the one choice that makes it intentional instead of generic.

### Choose minimal unless the surface is landing or marketing
Minimal mode starts from the key functionality and removes until the User loses clarity. Bold mode commits to one memorable direction; use `bold.md` when the surface needs that.
Never: add intensity to make a utility surface feel designed.

### Cut generic AI Slop aesthetics before coding
Reject gaudy or rainbow gradients, purposeless purple-blue-pink palettes, excessive rounded corners, motion that does not serve function, one-sided colored accent borders, divider lines between content sections, and lines used where hierarchy should work.
Example: separate sections with spacing, size, weight, and background.
Never: rely on `border-left: 3px solid` or `<hr>` to make hierarchy legible.

## 2. Put visual behavior in CSS

React changes data, state, and attributes. CSS owns visual state whenever CSS and data attributes can express it.

### Let CSS handle visual states
Disabled, hover, focus, active, loading, visibility, size, spacing, transitions, and animation belong in CSS. React owns data fetching, data-changing event handlers, conditional rendering of different components, and form submission logic.
Example: `<aside data-collapsed={isCollapsed}>` plus `.sidebar[data-collapsed="true"] { width: var(--sidebar-width-icon); }`.
Never: conditional `className` strings for visual states CSS can read from attributes.

### Use block element modifier classes with Tailwind applied inside them
Markup gets semantic classes. Tailwind supplies values inside CSS. Keep block element modifier depth to two levels; start a new block instead of writing a grandchild selector.
Example: `.sidebar`, `.sidebar__header`, `.sidebar--collapsed`.
Never: `block__element__sub-element`, `w-[--x]`, or Tailwind version 4 `&--modifier` nesting.

### Scope variables where they are used
Design-system variables live in `theme.css`; app timing and easing live in lean `global.css`; component variables live in the component CSS file; page overrides live in the page CSS file. A variable used by one component lives with that component.
Never: move a one-component variable into `theme.css`.

### Split layout from component appearance
When a structural slot contains a UI component, put both classes on the same element: the parent layout class owns grid area, placement, z-index, and position; the child component class owns background, padding, display, and gap.
Example: `<div class="frame__dock toolbar">` with `.frame__dock { grid-area: footer; }` and `.toolbar { display: flex; gap: 0.5rem; }`.
Never: grandchild selectors that make the parent know the child's internals.

## 3. Build layout, spacing, and type from the system

Every block uses layout primitives and scalable units so the surface adapts to its Context.

### Use grid or flex with gaps
Every block uses grid or flex, includes at least one dynamic-width column, and uses `gap` for spacing. Margins are only for sub-em optical corrections, `auto` alignment, or negative gap removal.
Example: `grid-template-columns: 1fr auto 1fr; gap: 1rem;`.
Never: margins as the primary layout spacing system.

### Prefer container queries for components
Container queries ask how much room the component has; media queries ask about the device. Use container queries for cards, grids, forms, galleries, tables, headers, navigation, and any component that appears in multiple Contexts.
Example: auto-declare the parent with `:has(> &) { container-type: inline-size; }` and use container units such as `cqi` or `cqb`.
Never: media queries for component adaptation. Media queries are for modals, off-canvas menus, fixed-position surfaces, print, and reduced motion.

### Size from scalable units
Priority is design tokens, then grid or flex with `ch` or `%`, then typography units such as `em`, `lh`, and `rlh`, then container units, then sub-em corrections. Use `rem` so browser font scaling works.
Never: pixels except for borders and box-shadow.

### Keep spacing on the Tailwind scale
Use the 4px grid in `rem`: `gap-1` through `gap-16`. Start generous around `2.5rem`, move related elements closer until they group, then snap to the scale. Use `ch` for horizontal text widths.
Never: invent component spacing variables unless the value is component-specific.

### Use three text sizes
Use `0.75rem` for captions, `0.875rem` for body and user interface text, and `1.125rem` for headings. Create hierarchy with weight and color, not more sizes.
Example: primary text at 90-100 percent lightness, secondary at 60-70 percent, disabled at 40-50 percent.
Never: add a fourth size to solve emphasis.

### Apply text polish deliberately
Use `text-wrap: balance` for headings and short text, `text-wrap: pretty` for body copy, neither for code. Apply `antialiased` at the root, use `tabular-nums` for dynamic numbers, keep paragraphs at 55ch maximum, and let line height carry vertical rhythm.

## 4. Use tokens for color, surfaces, radius, and contrast

Visual values come from the design system unless the codebase already owns a narrower component token.

### Derive colors from OKLCH tokens
Use tokens such as `var(--primary)`, `color-mix(in oklch, ...)`, and `oklch(from var(--primary) l c h / 50%)`. Generate palettes with 60 degree hue shifts; for dark and light pairs, `light L = 100 - dark L`.
Never: hardcoded colors in product CSS.

### Use gradients only as subtle texture
Gradients keep a surface from feeling flat; they must not become the content. Tailwind version 4 interpolates in oklab, which avoids muddy midpoints. A noise overlay at about `0.03` opacity can break digital smoothness.
Never: gaudy or high-saturation gradients.

### Pick a surface tier instead of inventing shadows
Use the surface tiers from `surfaces.md`: none, subtle, elevated, floating, and extruded. Every elevated tier includes a `0 0 0 1px` ring, an ambient shadow, and a contact shadow. The ring replaces the border.
Never: `z-index` without a shadow tier, invented `box-shadow` values, `border: 1px solid` surface boundaries, or Tailwind default shadow sizes.

### Use one radius system
Use `--radius: 0.675rem`; small elements use `calc(var(--radius) - 0.125rem)`, large containers use `calc(var(--radius) + 0.25rem)`. For nested rounded elements, outer radius equals inner radius plus padding, unless padding exceeds 24px.
Never: round every element by habit.

### Make optical corrections where geometry lies
Icon-and-text buttons need less padding on the icon side. Play triangles shift right. Large headings can use `letter-spacing: -0.02em`.
Example: `padding-inline-start: 0.75em; padding-inline-end: 1em;`.

### Check contrast in rem-sized text
Normal text needs at least 4.5:1 contrast. Large text and user interface elements need at least 3:1. In OKLCH, about `0.4` lightness difference usually passes.

## 5. Add motion only where state changes need it

Motion helps the User see the change, not just the result.

### Animate state changes unless skipping is a Decision
Use reactive motion for hover, press, and focus at 100-200ms; state-change motion for open, close, and swap at 200-400ms; and the View Transitions API for route or page changes. Skipping state-change motion requires a reason such as reduced motion or an interaction seen 100 or more times per day.
Never: let a modal pop in with no transition by default.

### Prefer CSS class and attribute transitions
CSS transitions are interruptible and should handle hover, dropdowns, toggles, reveals, and ordinary state changes. Use keyframes for one-shot sequences. Use JavaScript only for physics-driven drag, gestures, or the small reflow contracts in `transitions.md`.
Never: JavaScript animation libraries when CSS class toggles and intrinsic transitions can do the work.

### Animate intrinsic size instead of scale
For resizing surfaces, animate `width` and `height`, not `transform: scale()`, because scale distorts text, borders, and padding.
Never: `transform: scale()` for resize.

## 6. Verify the User experience before reporting done

The build does not catch design regressions.

### Check interaction patterns before polish
Buttons, forms, navigation, feedback, empty states, confirmation, accessibility, and modals use the contracts in `interactable.md`.
Example: destructive actions require confirmation with the safe action as primary; non-destructive actions get undo instead of confirmation.

### Run the checklist after the work
Use `checklist.md` after all design work and answer every item yes or no across CSS Architecture, sizing and layout, visual design, interactions, responsive behavior, accessibility, user experience patterns, and anti-patterns.
Never: report done before the checklist catches what build and lint miss.

## References (each solves one problem)

- Bold mode needs a memorable visual direction → bold.md
- Design work needs a yes/no Verification pass → checklist.md
- User interface Affordances need interaction contracts → interactable.md
- Motion needs timing, easing, state, accessibility, and performance constants → motion.md
- Surfaces need elevation tiers and shadow recipes → surfaces.md
- State changes need CSS-first recipes → transitions.md

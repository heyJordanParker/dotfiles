---
name: design-thinking
description: Use when building UI components, styling pages, or making visual/interaction decisions. Guides design thinking, anti-patterns, CSS architecture, and responsive patterns.
---

# Frontend Design

Use when building UI components, styling pages, or making visual/interaction decisions.

## Design Thinking (Before Code)

**Good design is as little design as possible.**

Ask: What's the key functionality? Start from there. Fewer colors, fewer words, less clutter.

1. **Purpose** - What does this accomplish for the user?
2. **Tone** - Professional? Playful? Minimal? Bold?
3. **Constraints** - Space available? Existing patterns to match?
4. **Differentiation** - What makes this feel intentional, not generic?

**Intentionality > Intensity.** Every choice should have a reason.

## Anti-Patterns

Never create generic AI aesthetics:
- Gaudy, high-saturation, or rainbow gradients (subtle gradients that add texture are good)
- Excessive rounded corners on everything
- Purple-blue-pink color schemes with no purpose
- Animations that don't serve function
- "Modern" for modern's sake

If it looks like every AI-generated landing page, redo it.

## Core Principles

- **Simplicity wins** - Remove until it breaks, then add one thing back
- **Hierarchy through restraint** - One focal point per view
- **Consistency > novelty** - Match existing patterns before inventing
- **Function drives form** - Every visual choice serves usability

## Quick Reference

**Spacing:** 4px grid in rem (0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 2.5, 3, 4rem)

**Typography:** 3 sizes (0.75, 0.875, 1.125rem) + weight/color for hierarchy

**Timing:** hover=200ms, active=instant, modal=300ms

**Easing:** `cubic-bezier(0.4, 0, 0.2, 1)`

## References

- [css-architecture.md](references/css-architecture.md) - File structure, BEM, Tailwind @apply, variable scoping
- [visual-design.md](references/visual-design.md) - Colors (OKLCH), spacing, typography, shadows, elevation
- [interactions.md](references/interactions.md) - Timing, easing, states, keyframes, micro-interactions
- [ux-patterns.md](references/ux-patterns.md) - Forms, navigation, feedback, accessibility, modals
- [responsive.md](references/responsive.md) - Desktop-first breakpoints, touch detection, admin patterns

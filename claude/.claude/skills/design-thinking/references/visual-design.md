# Visual Design

## Spacing

**4px grid in rem.** Divide by 16 to convert:

| px | rem | Tailwind |
|----|-----|----------|
| 4 | 0.25rem | gap-1, p-1 |
| 8 | 0.5rem | gap-2, p-2 |
| 12 | 0.75rem | gap-3, p-3 |
| 16 | 1rem | gap-4, p-4 |
| 20 | 1.25rem | gap-5, p-5 |
| 24 | 1.5rem | gap-6, p-6 |
| 32 | 2rem | gap-8, p-8 |
| 40 | 2.5rem | gap-10, p-10 |
| 48 | 3rem | gap-12, p-12 |
| 64 | 4rem | gap-16, p-16 |

**Process:**
1. Start generous (2.5rem / 40px)
2. Bring elements closer until they feel grouped
3. Pick from the scale

**Use Tailwind utilities** for spacing: `gap-4`, `p-6`, `m-2`. Define CSS variables only for component-specific values.

## Typography

**3 sizes only (rem):**

| rem | px | Tailwind | Use |
|-----|----|---------|----|
| 0.75rem | 12px | text-xs | Captions, metadata |
| 0.875rem | 14px | text-sm | Body text, UI (base) |
| 1.125rem | 18px | text-lg | Headings, emphasis |

**Hierarchy through weight and color, not size.**

To emphasize an element, de-emphasize everything else. You can't make white "more white" - instead reduce the lightness of secondary text.

| Role | Lightness (dark mode) | Tailwind |
|------|----------------------|----------|
| Primary (titles) | 90-100% | text-foreground |
| Secondary | 60-70% | text-muted-foreground |
| Disabled/hint | 40-50% | text-muted-foreground/50 |

**Line height as spacing.** Greater line height acts as natural margin-bottom. In most cases you don't need manual gap between text blocks - line height handles it.

## Colors (OKLCH)

OKLCH is perceptually uniform - colors with same lightness actually look equally bright.

```css
/* Format: oklch(lightness chroma hue) */
--primary: oklch(0.64 0.17 36);  /* Warm orange */
```

**Palette generation with 60° hue shifts:**
```css
:root {
  --hue: 36;  /* Base hue */
  --primary: oklch(0.64 0.17 var(--hue));
  --secondary: oklch(0.55 0.12 var(--hue));
  --tertiary: oklch(0.64 0.15 calc(var(--hue) - 60));  /* 60° left */
  --accent: oklch(0.64 0.15 calc(var(--hue) + 60));    /* 60° right */
}
```

This creates a 120° arc - the same distance between primary colors on the wheel.

**Dark/light mode conversion:** `Light mode lightness = 100 - Dark mode lightness`

**Hierarchy:**
- Background: Very low chroma (nearly gray)
- Text: No chroma (pure gray) or very low
- Accents: Higher chroma for emphasis

## Elevation & Shadows

**Light source is at the top.** Top surfaces are lighter, bottom surfaces are darker.

| Level | Background | Shadow | Use |
|-------|------------|--------|-----|
| 0 | Page base | None | Content areas |
| 1 | Slightly lifted | shadow-xs | Sidebar body, cards |
| 2 | Floating | shadow-sm | Sticky headers, glass panels |
| 3 | Overlay | shadow-md | Dropdowns, modals |

**Dual shadow system (soft + dark):**

Combine two shadow types for realistic depth:
1. **Light edge on top** - simulates light hitting elevated surface
2. **Dark shadow at bottom** - the actual shadow cast

```css
box-shadow:
  inset 0 1px 0 rgba(255,255,255,0.05),  /* Light edge top */
  0 4px 12px rgba(0,0,0,0.03),            /* Soft ambient */
  0 1px 3px rgba(0,0,0,0.06);             /* Sharp contact */
```

**Recessed elements** (inputs, wells): Dark inset shadow on top + light inset shadow on bottom.

**Rules:**
- Elevated = lighter background + more shadow
- Never use z-index without corresponding shadow
- Glass effect: `backdrop-blur-md` + semi-transparent bg + layered shadow

## Radius

One radius value: `--radius: 0.675rem`

Variants derived from it:
- Small elements: `calc(var(--radius) - 0.125rem)`
- Large containers: `calc(var(--radius) + 0.25rem)`

Don't round everything. Intentional use only.

## Contrast & Accessibility

**Minimum contrast ratios:**
- Normal text: 4.5:1
- Large text (1.125rem+ bold or 1.5rem+): 3:1
- UI components: 3:1

**OKLCH shortcut:** Lightness difference of ~0.4 usually passes.

**Use rem for accessibility.** Users can adjust browser font size - rem respects this, px doesn't.

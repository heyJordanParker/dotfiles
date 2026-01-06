# Responsive Design

## Desktop-First (Admin UI)

This is an admin interface. Desktop is the primary experience. Mobile is functional but degraded.

**Write styles for desktop first, then adapt down:**
```css
.dashboard-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);

  @media (max-width: 1024px) {
    grid-template-columns: repeat(2, 1fr);
  }

  @media (max-width: 768px) {
    grid-template-columns: 1fr;
  }
}
```

## Breakpoints

| Name | Width | Target |
|------|-------|--------|
| Wide | 1440px+ | Large monitors |
| Desktop | 1024px+ | Standard laptops |
| Tablet | 768px+ | iPad landscape |
| Mobile | <768px | Phones (degraded) |

**Tailwind (desktop-first):**
```
Default = desktop
lg:  < 1024px
md:  < 768px
sm:  < 640px
```

Note: Tailwind defaults to mobile-first. For desktop-first, use max-width media queries in CSS or consider if the complexity is worth it.

## Touch Detection

**Desktop-only hover effects:**
```css
@media (hover: hover) {
  .card:hover {
    @apply shadow-md -translate-y-1;
  }
}
```

`(hover: hover)` = device has a pointer that can hover (mouse, trackpad).
`(hover: none)` = touch-only device.

**Touch targets:**
- Minimum 44x44px for touch interactions
- More spacing between clickable items on mobile
- Larger buttons on tablet

## Admin-Specific Patterns

**Sidebar:**
- Desktop: persistent sidebar (16rem)
- Mobile: collapsible or hidden, hamburger trigger

**Tables:**
- Desktop: full table with all columns
- Tablet: horizontal scroll or hide less important columns
- Mobile: card layout or horizontal scroll

**Forms:**
- Desktop: can use side-by-side layouts if needed
- Mobile: always single column

**Complex features:**
- Some admin features may simply not work well on mobile
- That's acceptable - show a message directing to desktop
- Don't degrade the desktop experience for mobile parity

## What NOT to Do

- Don't hide critical functionality on desktop for "clean" mobile
- Don't mobile-first when desktop is the primary use case
- Don't add complexity for edge-case mobile scenarios
- Don't test mobile responsiveness before desktop is solid

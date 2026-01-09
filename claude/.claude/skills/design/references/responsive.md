# Responsive Design

## Container Queries First

Container queries ask "how much room do I have?" instead of "how big is the screen?" Components adapt to their context, not device size.

**Use container queries for:**
- Cards, grids, forms, galleries, tables
- Headers, footers, navigation
- Any component that could exist in multiple contexts

**Use media queries only for:**
- Modals, off-canvas menus
- Fixed-position elements
- Device-specific concerns (print, reduced-motion)

## The "Has Me" Pattern

Container queries require `container-type` on the parent. Instead of manual setup, auto-declare from the child:

```css
.card {
  /* Auto-declare parent as container */
  :has(> &) {
    container-type: inline-size;
  }

  /* Now use container queries */
  @container (min-width: 400px) {
    display: grid;
    grid-template-columns: 200px 1fr;
  }
}
```

This makes container declarations travel with components - no manual sync needed.

## Grid Item Wrapper Requirement

Grid items need physical "cells" for container queries to measure. Use semantic wrappers:

```html
<!-- Right: list items provide the container boundary -->
<ul class="grid">
  <li>
    <article class="card">...</article>
  </li>
</ul>
```

```css
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 1rem;
}

.grid > li {
  container-type: inline-size;
}

.card {
  @container (min-width: 300px) {
    /* horizontal layout */
  }
}
```

## Container Units

- **`cqi`:** 1% of container's inline size (width in horizontal writing)
- **`cqb`:** 1% of container's block size (height)
- **`cqmin`:** Smaller of cqi/cqb
- **`cqmax`:** Larger of cqi/cqb

Prefer `cqi`/`cqb` over viewport units (`vw`/`vh`) and media queries.

## Touch Detection

Desktop-only hover effects:

```css
@media (hover: hover) {
  .card:hover {
    @apply shadow-md -translate-y-1;
  }
}
```

Touch targets: minimum 44x44px for touch interactions.

## When Media Queries Are Appropriate

```css
/* Print styles */
@media print {
  .no-print { display: none; }
}

/* Reduced motion preference */
@media (prefers-reduced-motion: reduce) {
  * { animation-duration: 0.01ms !important; }
}

/* Off-canvas menu - device-specific */
@media (max-width: 768px) {
  .sidebar { transform: translateX(-100%); }
}
```

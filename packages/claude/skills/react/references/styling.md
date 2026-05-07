# Styling Best Practices

## Use Tailwind CSS or CSS Modules for New Projects

Both are zero-runtime and fully compatible with React Server Components. Runtime CSS-in-JS (styled-components, Emotion) adds 11-28 KB bundle overhead and conflicts with streaming SSR.

**CSS Modules** — scoped CSS with `.module.css` extension, built into Vite and Next.js:

```tsx
import styles from './Button.module.css'

function Button({ children }) {
  return <button className={styles.primary}>{children}</button>
}
```

**Tailwind CSS** — utility-first classes in markup, purged at build time:

```tsx
function Button({ children }) {
  return <button className="rounded bg-blue-600 px-4 py-2 text-white">{children}</button>
}
```

## cn() Utility for Conditional Classes

Combine clsx (conditional joining) with tailwind-merge (conflict resolution). This is the foundation of Tailwind + React:

```tsx
import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
```

```tsx
function Button({ variant, className, children }) {
  return (
    <button className={cn(
      'rounded px-4 py-2 font-medium',
      variant === 'primary' && 'bg-blue-600 text-white',
      variant === 'secondary' && 'bg-gray-200 text-gray-900',
      className
    )}>
      {children}
    </button>
  )
}
```

## CVA for Variant Systems

class-variance-authority provides a declarative API for component variants with type-safe props:

```tsx
import { cva, type VariantProps } from 'class-variance-authority'

const button = cva('inline-flex items-center rounded font-medium', {
  variants: {
    intent: {
      primary: 'bg-blue-600 text-white hover:bg-blue-700',
      secondary: 'bg-gray-200 text-gray-900 hover:bg-gray-300',
      danger: 'bg-red-600 text-white hover:bg-red-700',
    },
    size: {
      sm: 'px-2 py-1 text-sm',
      md: 'px-4 py-2 text-base',
      lg: 'px-6 py-3 text-lg',
    },
  },
  defaultVariants: { intent: 'primary', size: 'md' },
})

type ButtonProps = VariantProps<typeof button> & { className?: string }

function Button({ intent, size, className, children }: ButtonProps) {
  return <button className={cn(button({ intent, size }), className)}>{children}</button>
}
```

## Never Concatenate Tailwind Classes Dynamically

Tailwind's compiler scans source files for complete class strings. Dynamic concatenation produces classes the compiler can't detect, so they get purged from production CSS.

**Incorrect:**

```tsx
<div className={`bg-${color}-500 text-${size}`} />
```

**Correct:**

```tsx
<div className={cn(
  color === 'blue' && 'bg-blue-500',
  color === 'red' && 'bg-red-500',
  size === 'lg' && 'text-lg',
)} />
```

## CSS Custom Properties for Design Tokens

Define tokens as CSS custom properties on `:root`. Components reference variables, not hard-coded values. Use `@layer` for precedence control.

```css
@layer reset, tokens, base, components, utilities;

@layer tokens {
  :root {
    --color-primary: oklch(0.55 0.24 262);
    --color-surface: oklch(0.98 0.003 264);
    --color-text: oklch(0.2 0.02 264);
    --space-sm: 8px;
    --space-md: 16px;
    --space-lg: 24px;
  }
}
```

Global styles define tokens, resets, and base typography only. All component styling should be scoped.

## Server Component Compatibility

- CSS Modules — RSC compatible, 0 KB runtime
- Tailwind CSS — RSC compatible, 0 KB runtime
- vanilla-extract — RSC compatible, 0 KB runtime
- StyleX — RSC compatible, 0 KB runtime
- styled-components v6.3+ — RSC compatible, ~11 KB runtime
- Emotion — NOT RSC compatible, ~11 KB runtime

Runtime CSS-in-JS relies on React Context and runtime style injection — both unavailable in Server Components. styled-components added RSC support in v6.3.0 (Jan 2026), but dynamic interpolations incur serialization overhead.

## Don't Start New Projects with Runtime CSS-in-JS

styled-components is in maintenance mode. Emotion receives no new features. For existing codebases, styled-components v6.3+ works with RSC — migration isn't urgent. For new projects, use Tailwind, CSS Modules, or vanilla-extract.

## Dark Mode

Use CSS custom properties with a `[data-theme]` selector. Add a blocking script before React hydrates to prevent the flash of wrong theme.

```css
:root {
  --color-bg: oklch(0.98 0.003 264);
  --color-text: oklch(0.2 0.02 264);
}

[data-theme="dark"] {
  --color-bg: oklch(0.15 0.02 264);
  --color-text: oklch(0.92 0.01 264);
}
```

**Blocking script in `<head>` (before any CSS):**

```html
<script>
  const theme = localStorage.getItem('theme') ||
    (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  document.documentElement.setAttribute('data-theme', theme);
</script>
```

**Three-way toggle (system / light / dark):** Store user preference in localStorage, resolve against system settings, apply resolved theme to DOM.

## Decision Framework

- **Tailwind CSS** — default for most projects. Zero runtime, excellent DX, huge ecosystem (shadcn/ui)
- **CSS Modules** — teams preferring traditional CSS. Zero config, familiar, scoped by default
- **vanilla-extract** — TypeScript-first design systems. Type-safe tokens, Sprinkles, theme contracts
- **StyleX** — Meta-scale codebases. Atomic deduplication reduced CSS by 80% at Meta

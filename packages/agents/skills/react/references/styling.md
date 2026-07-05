# Styling

One Process: choose zero-runtime styling first, merge conditional classes safely, encode tokens as CSS variables, and preserve User theme preference before hydration.

## 1. Choose the styling system

### Use Tailwind CSS or CSS Modules for new projects

Both are zero-runtime and compatible with React Server Components. Runtime CSS-in-JS adds 11 to 28 kilobytes of bundle overhead and conflicts with streaming server rendering.

Example with CSS Modules:
  ```tsx
  import styles from './Button.module.css'

  function Button({ children }) {
    return <button className={styles.primary}>{children}</button>
  }
  ```

Example with Tailwind CSS:
  ```tsx
  function Button({ children }) {
    return <button className="rounded bg-blue-600 px-4 py-2 text-white">{children}</button>
  }
  ```

### Do not start new projects with runtime CSS-in-JS

`styled-components` is in maintenance mode. Emotion receives no new features. Existing codebases using `styled-components` v6.3 and later can work with React Server Components, so migration is not urgent. New projects use Tailwind CSS, CSS Modules, or `vanilla-extract`.

## 2. Merge classes through one helper

### Use cn for conditional classes

Combine `clsx` for conditional joining with `tailwind-merge` for conflict resolution.

Example:
  ```tsx
  import { clsx, type ClassValue } from 'clsx'
  import { twMerge } from 'tailwind-merge'

  export function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs))
  }
  ```

Example:
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

### Never concatenate Tailwind classes dynamically

Tailwind's compiler scans source files for complete class strings. Dynamic concatenation produces classes the compiler cannot detect, so they are purged from production CSS.

Never:
  ```tsx
  <div className={`bg-${color}-500 text-${size}`} />
  ```

Example:
  ```tsx
  <div className={cn(
    color === 'blue' && 'bg-blue-500',
    color === 'red' && 'bg-red-500',
    size === 'lg' && 'text-lg',
  )} />
  ```

## 3. Use a variant helper when variants grow

### Use class-variance-authority for variant systems

`class-variance-authority` provides a declarative API for component variants with type-safe props.

Example:
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

## 4. Put style tokens in CSS custom properties

### Define tokens on root and scope component styling

Components reference variables, not hard-coded values. Use `@layer` for precedence control. Global styles define tokens, resets, and base typography only; component styling stays scoped.

Example:
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

## 5. Check Server Component compatibility

### Prefer zero-runtime options at server boundaries

CSS Modules, Tailwind CSS, `vanilla-extract`, and StyleX are React Server Component compatible with zero kilobytes of runtime. `styled-components` v6.3 and later is compatible with about 11 kilobytes of runtime. Emotion is not React Server Component compatible and adds about 11 kilobytes of runtime.

Runtime CSS-in-JS relies on React Context and runtime style injection, both unavailable in Server Components. `styled-components` added React Server Component support in v6.3.0 in January 2026, but dynamic interpolations incur serialization overhead.

## 6. Preserve theme before hydration

### Use CSS custom properties for dark mode

Use CSS custom properties with a `[data-theme]` selector. Add a blocking script before React hydrates to prevent the flash of the wrong theme.

Example:
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

Example blocking script in `<head>` before any CSS:
  ```html
  <script>
    const theme = localStorage.getItem('theme') ||
      (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    document.documentElement.setAttribute('data-theme', theme);
  </script>
  ```

For a system, light, and dark toggle, store User preference in localStorage, resolve against system settings, and apply the resolved theme to the DOM.

## 7. Pick from the surviving options

### Tailwind CSS is the default for most projects

It has zero runtime and a large ecosystem such as `shadcn/ui`.

### CSS Modules fit teams that prefer traditional CSS

They have zero configuration and are scoped by default.

### vanilla-extract fits TypeScript-first component systems

It gives type-safe tokens, Sprinkles, and theme contracts.

### StyleX fits Meta-scale codebases

Measured at Meta: atomic deduplication reduced CSS by 80 percent.

# TypeScript Patterns for React

## Discriminated Union Props

Use discriminated unions for exclusive prop sets instead of optional props. TypeScript narrows the type based on the discriminant.

**Incorrect:**
```tsx
type ModalProps = { variant: "no-title" | "title"; title?: string }
```

**Correct:**
```tsx
type ModalProps =
  | { variant: "no-title" }
  | { variant: "title"; title: string }
```

For one-or-the-other exclusivity without a discriminant, use `never`:
```tsx
type Props =
  | { foo: string; bar?: never }
  | { bar: string; foo?: never }
```

## ComponentPropsWithoutRef for Extending HTML Elements

Prefer `ComponentPropsWithoutRef` over `HTMLAttributes` when wrapping native elements. Use `Omit` to resolve conflicting prop names.

```tsx
interface ButtonProps extends React.ComponentPropsWithoutRef<'button'> {
  variant?: 'primary' | 'secondary'
  isLoading?: boolean
}

function Button({ variant, isLoading, ...rest }: ButtonProps) {
  return <button {...rest} disabled={isLoading || rest.disabled} />
}
```

Extract props from components you don't control: `type ModalProps = React.ComponentProps<typeof Modal>`

## Event Handler Typing

Inline handlers are auto-inferred. Separate handler functions need explicit typing.

```tsx
// Inline — auto-inferred
<input onChange={(e) => { /* e is React.ChangeEvent<HTMLInputElement> */ }} />

// Separate — type the parameter
const handleChange = (e: React.ChangeEvent<HTMLInputElement>): void => {
  console.log(e.currentTarget.value)
}

// Or type the function
const handleChange: React.ChangeEventHandler<HTMLInputElement> = (e) => {
  console.log(e.currentTarget.value)
}
```

Common types: `ChangeEvent`, `MouseEvent`, `KeyboardEvent`, `FocusEvent`, `FormEvent`, `PointerEvent`, `SyntheticEvent` (fallback). React 19.2+ deprecates `FormEvent` in favor of `SubmitEvent`.

## createContext with Null and Custom Hook Guard

Create context with null default and a guard hook. Eliminates null checks at every usage site.

```tsx
const AuthContext = createContext<AuthContextType | null>(null)

function useAuth(): AuthContextType {
  const context = useContext(AuthContext)
  if (!context) throw new Error("useAuth must be used within AuthProvider")
  return context
}
```

Avoid type assertions like `createContext<T>({} as T)` — they skip runtime safety.

## useState and useRef Typing

Let TypeScript infer when possible. Use explicit generics for unions or null.

```tsx
const [count, setCount] = useState(0)                            // inferred: number
const [user, setUser] = useState<User | null>(null)              // explicit: union
const [status, setStatus] = useState<'idle' | 'loading'>('idle') // explicit: literal

// DOM refs — pass null, get read-only RefObject (must null-check)
const divRef = useRef<HTMLDivElement>(null)

// Mutable value refs — pass initial value, get MutableRefObject
const intervalRef = useRef<number | null>(null)
```

## useReducer with Discriminated Union Actions

Use discriminated unions for action types with a `never` exhaustive check in the default case.

```tsx
type Action =
  | { type: "increment"; payload: number }
  | { type: "decrement"; payload: number }
  | { type: "reset" }

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "increment": return { count: state.count + action.payload }
    case "decrement": return { count: state.count - action.payload }
    case "reset": return initialState
    default:
      const _exhaustive: never = action
      throw new Error(`Unknown action: ${_exhaustive}`)
  }
}
```

## Plain Functions Over React.FC

React.FC is technically fine since TypeScript 5.1 (implicit children and return type issues were fixed), but plain functions remain preferred: simpler, easier to refactor to generic, standard TypeScript syntax.

**Preferred:**
```tsx
function Greeting({ name }: { name: string }) {
  return <div>Hello {name}</div>
}
```

Generic components are impossible with React.FC but natural with plain functions:
```tsx
function List<T>({ items, renderItem }: { items: T[]; renderItem: (item: T) => ReactNode }) {
  return <ul>{items.map(renderItem)}</ul>
}
```

## as const for Tuples and String Unions

Use `as const` to prevent type widening. Essential for hook return tuples, string union derivation, and enum alternatives.

```tsx
// Hook tuple — without as const, returns (boolean | () => void)[]
function useToggle(initial = false) {
  const [value, setValue] = useState(initial)
  const toggle = useCallback(() => setValue(v => !v), [])
  return [value, toggle] as const // readonly [boolean, () => void]
}

// String union from array
const ROLES = ["admin", "editor", "viewer"] as const
type Role = (typeof ROLES)[number] // "admin" | "editor" | "viewer"
```

## satisfies for Type-Safe Configs

`satisfies` validates structure without widening types. Combine with `as const` for full safety: validated, literal, readonly.

```tsx
export const flags = {
  newNavbar: true, betaSignup: false,
} as const satisfies { newNavbar: boolean; betaSignup: boolean }
// flags.newNavbar is `true` (literal), typos caught at compile time
```

## Generic Components

Use generics for reusable type-safe components. Arrow functions need `extends unknown` to avoid JSX ambiguity.

```tsx
function Select<T>(props: { options: T[]; value: T; onChange: (v: T) => void }) { /* ... */ }

// Arrow syntax — must constrain to disambiguate from JSX
const Select = <T extends unknown>(props: SelectProps<T>) => { /* ... */ }
```

## ReactNode for Children

Use `ReactNode` for children 99% of the time. It accepts strings, numbers, elements, arrays, null, and fragments. `ReactElement` is narrower (JSX only). You cannot enforce specific component types as children.

## React 19: ref as Regular Prop

React 19 deprecates `forwardRef`. Pass `ref` directly as a prop using `Ref<T>` type.

```tsx
// React 19 — ref is just a prop
function Input({ ref, ...props }: InputProps & { ref?: Ref<HTMLInputElement> }) {
  return <input ref={ref} {...props} />
}
```

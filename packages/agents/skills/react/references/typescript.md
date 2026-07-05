# TypeScript

One Process: make invalid prop states unrepresentable, rely on inference where it holds, and encode reusable React types at the boundary.

## 1. Model exclusive props with the type system

### Use discriminated unions for exclusive prop sets

Discriminated unions let TypeScript narrow the type based on the discriminant.

Never:
  ```tsx
  type ModalProps = { variant: "no-title" | "title"; title?: string }
  ```

Example:
  ```tsx
  type ModalProps =
    | { variant: "no-title" }
    | { variant: "title"; title: string }
  ```

### Use never for one-or-the-other props without a discriminant

Example:
  ```tsx
  type Props =
    | { foo: string; bar?: never }
    | { bar: string; foo?: never }
  ```

## 2. Type wrappers at the native element boundary

### Use ComponentPropsWithoutRef for native wrappers

Prefer `ComponentPropsWithoutRef` over `HTMLAttributes` when wrapping native elements. Use `Omit` to resolve conflicting prop names.

Example:
  ```tsx
  interface ButtonProps extends React.ComponentPropsWithoutRef<'button'> {
    variant?: 'primary' | 'secondary'
    isLoading?: boolean
  }

  function Button({ variant, isLoading, ...rest }: ButtonProps) {
    return <button {...rest} disabled={isLoading || rest.disabled} />
  }
  ```

Extract props from components you do not control with `type ModalProps = React.ComponentProps<typeof Modal>`.

## 3. Type event handlers only when inference cannot

### Let inline handlers infer types

Inline handlers are auto-inferred. Separate handler functions need explicit typing.

Example:
  ```tsx
  <input onChange={(event) => { /* event is React.ChangeEvent<HTMLInputElement> */ }} />
  ```

Example:
  ```tsx
  const handleChange = (event: React.ChangeEvent<HTMLInputElement>): void => {
    console.log(event.currentTarget.value)
  }

  const handleChangeTyped: React.ChangeEventHandler<HTMLInputElement> = (event) => {
    console.log(event.currentTarget.value)
  }
  ```

Common event types: `ChangeEvent`, `MouseEvent`, `KeyboardEvent`, `FocusEvent`, `FormEvent`, `PointerEvent`, and `SyntheticEvent` as fallback. React 19.2 and later deprecates `FormEvent` in favor of `SubmitEvent`.

## 4. Guard context at runtime

### Create context with null and a custom hook guard

A null default plus a guard hook eliminates null checks at every usage site.

Example:
  ```tsx
  const AuthContext = createContext<AuthContextType | null>(null)

  function useAuth(): AuthContextType {
    const context = useContext(AuthContext)
    if (!context) throw new Error("useAuth must be used within AuthProvider")
    return context
  }
  ```

Never: `createContext<T>({} as T)` because it skips runtime safety.

## 5. Use inference until a union or null needs help

### Type useState and useRef by need

Let TypeScript infer simple values. Use explicit generics for unions or null.

Example:
  ```tsx
  const [count, setCount] = useState(0)
  const [user, setUser] = useState<User | null>(null)
  const [status, setStatus] = useState<'idle' | 'loading'>('idle')

  const divRef = useRef<HTMLDivElement>(null)
  const intervalRef = useRef<number | null>(null)
  ```

## 6. Make reducers exhaustive

### Use discriminated union actions with a never check

Example:
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

## 7. Prefer plain component functions

### Use plain functions over React.FC

`React.FC` is technically fine since TypeScript 5.1 fixed implicit children and return type issues, but plain functions remain simpler, easier to refactor to generics, and standard TypeScript syntax.

Example:
  ```tsx
  function Greeting({ name }: { name: string }) {
    return <div>Hello {name}</div>
  }
  ```

Example:
  ```tsx
  function List<T>({ items, renderItem }: { items: T[]; renderItem: (item: T) => ReactNode }) {
    return <ul>{items.map(renderItem)}</ul>
  }
  ```

## 8. Preserve literal types where needed

### Use as const for tuples and string unions

`as const` prevents type widening. It is essential for hook return tuples, string union derivation, and enum alternatives.

Example:
  ```tsx
  function useToggle(initial = false) {
    const [value, setValue] = useState(initial)
    const toggle = useCallback(() => setValue(value => !value), [])
    return [value, toggle] as const
  }

  const ROLES = ["admin", "editor", "viewer"] as const
  type Role = (typeof ROLES)[number]
  ```

### Use satisfies for type-safe configs

`satisfies` validates shape without widening types. Combine it with `as const` for validated, literal, readonly config.

Example:
  ```tsx
  export const flags = {
    newNavbar: true, betaSignup: false,
  } as const satisfies { newNavbar: boolean; betaSignup: boolean }
  ```

## 9. Use generics for reusable components

### Disambiguate generic arrow components

Use generics for reusable type-safe components. Arrow functions need `extends unknown` to avoid JSX ambiguity.

Example:
  ```tsx
  function Select<T>(props: { options: T[]; value: T; onChange: (value: T) => void }) { /* ... */ }

  const SelectArrow = <T extends unknown>(props: SelectProps<T>) => { /* ... */ }
  ```

## 10. Type children broadly

### Use ReactNode for children

Use `ReactNode` for children 99 percent of the time. It accepts strings, numbers, elements, arrays, null, and fragments. `ReactElement` is narrower. You cannot enforce specific component types as children.

## 11. Use React 19 ref props

### Pass ref as a regular prop

React 19 deprecates `forwardRef`. Pass `ref` directly as a prop using the `Ref<T>` type.

Example:
  ```tsx
  function Input({ ref, ...props }: InputProps & { ref?: Ref<HTMLInputElement> }) {
    return <input ref={ref} {...props} />
  }
  ```

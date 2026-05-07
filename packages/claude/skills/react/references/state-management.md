# State Management and Custom Hooks

## State Colocation

Place state as close to where it's used as possible. State at the top of the tree forces React to check every component on every update. Colocated state lets React skip unaffected branches.

**Decision tree:**

1. Used only by this component? Keep it here
2. Used only by one child? Move it down to that child
3. Used by siblings? Lift to lowest common parent
4. Prop drilling painful? Use Context (positioned close, not at root)

Push state back down during refactoring — lifted state that's no longer shared should be colocated again.

## Lift State to Lowest Common Parent

When siblings need shared state, lift it to their nearest common ancestor. Don't lift higher than necessary.

**Incorrect:**

```tsx
// State in App for two sibling inputs — affects entire tree
function App() {
  const [filter, setFilter] = useState('')
  return <Layout><Sidebar filter={filter} /><Content filter={filter} setFilter={setFilter} /></Layout>
}
```

**Correct:**

```tsx
// State in the shared parent only
function FilteredContent() {
  const [filter, setFilter] = useState('')
  return <><FilterInput value={filter} onChange={setFilter} /><Results filter={filter} /></>
}
```

## When Context Is Enough

Context works well for read-heavy, write-rare data. Good use cases: theme, auth, locale, i18n.

**Separate state and dispatch contexts** to prevent unnecessary re-renders:

```tsx
const TasksContext = createContext<Task[]>(null)
const TasksDispatchContext = createContext<Dispatch>(null)

function TasksProvider({ children }) {
  const [tasks, dispatch] = useReducer(tasksReducer, initialTasks)
  return (
    <TasksContext value={tasks}>
      <TasksDispatchContext value={dispatch}>{children}</TasksDispatchContext>
    </TasksContext>
  )
}

export function useTasks() { return use(TasksContext) }
export function useTasksDispatch() { return use(TasksDispatchContext) }
```

Components that only dispatch never re-render when tasks change.

## Server State vs Client State

Server cache and UI state are fundamentally different problems. Mixing them creates bugs.

**Server state** — data owned by the server, cached on the client. Use TanStack Query or SWR:

```tsx
function UserProfile({ userId }) {
  const { data, isLoading, error } = useQuery({
    queryKey: ['user', userId],
    queryFn: () => fetchUser(userId),
  })
}
```

**Client state** — data owned by the browser/UI (modal open, selected tab, form draft). Use useState, useReducer, or Zustand.

**Recommended stack:** TanStack Query (server) + nuqs (URL) + useState/Zustand (client).

## URL State

Use nuqs for shareable filter, sort, and pagination state. Manual URL syncing creates bugs.

```tsx
import { useQueryState } from 'nuqs'

function SearchPage() {
  const [query, setQuery] = useQueryState('q')
  return <input value={query ?? ''} onChange={(e) => setQuery(e.target.value)} />
}
```

Use URL state when the value should survive page refresh, be bookmarkable, or be shareable via link.

## When to Reach for External Stores

Context struggles with fine-grained subscriptions — every consumer re-renders when any part of the context value changes. Reach for Zustand or Jotai when you need:

- Fine-grained subscriptions (component only re-renders on its slice)
- Complex shared state with frequent updates
- State access outside React (event handlers, utilities)

```tsx
import { create } from 'zustand'

const useStore = create((set) => ({
  count: 0,
  increment: () => set((state) => ({ count: state.count + 1 })),
}))

function Counter() {
  const count = useStore((state) => state.count) // Only re-renders when count changes
}
```

## Global State Anti-Patterns

**Everything global.** Most state is local UI state. Only auth, theme, and locale belong in global stores. Putting everything global forces unrelated components to re-render.

**Context for frequently changing values.** Mouse position, scroll, rapid typing — these must not go in Context. Every consumer re-renders on every change. Use Zustand or Jotai for fine-grained subscriptions.

**Monolithic provider.**

```tsx
// Every change to any field re-renders everything
<AppContext.Provider value={{ user, theme, notifications, cart, locale }}>
```

Split into separate providers or use an external store with selectors.

**Global state as server cache.** Storing API responses in Redux or Zustand means manually handling caching, staleness, and invalidation. Use TanStack Query instead.

## Form State

**Uncontrolled forms (prefer for React 19):**

```tsx
const [state, formAction, isPending] = useActionState(
  async (prevState, formData) => {
    const result = await submitForm(formData)
    if (result.error) return { error: result.error }
    return { success: true }
  },
  { error: null }
)

return (
  <form action={formAction}>
    <input name="email" />
    <SubmitButton />
    {state.error && <p>{state.error}</p>}
  </form>
)
```

React auto-resets uncontrolled forms on success. Use `useFormStatus` for pending state without prop drilling.

**Controlled forms** — when you need real-time validation, conditional fields, or live preview. Use react-hook-form for complex controlled forms.

## Custom Hook Naming

Hook names must start with `use` + capital letter (linter-enforced). Name after purpose, not lifecycle. Functions that don't call hooks should NOT use `use` prefix — regular functions can be called conditionally.

```tsx
// Incorrect
function useMount(fn) { useEffect(() => { fn() }, []) }  // lifecycle wrapper
function useSorted(items) { return items.slice().sort() }  // no hooks inside

// Correct
function useChatRoom({ serverUrl, roomId }) { /* ... */ }  // named after purpose
function getSorted(items) { return items.slice().sort() }   // regular function
```

## When to Extract a Custom Hook

Extract when logic is duplicated across components, synchronizes with an external system, or when a complex effect would benefit from clearer intent. Don't extract when it's 1-2 lines or doesn't call hooks.

```tsx
// Before: unclear what the effect does
useEffect(() => {
  const conn = createConnection({ serverUrl, roomId })
  conn.connect()
  return () => conn.disconnect()
}, [roomId, serverUrl])

// After: clear intent
useChatRoom({ roomId, serverUrl })
```

## Return Value Conventions

- **Single value:** `const isOnline = useOnlineStatus()`
- **Tuple:** `const [selected, onNext] = useList(products)` — for useState-like patterns
- **Object:** `const { value, onChange } = useFormInput('Mary')` — for 3+ values, partial destructuring, or growable APIs

## Always Wrap Returned Functions in useCallback

Consumers may pass returned functions as props to memoized children or use them as effect dependencies. Without useCallback, every render creates new references.

```tsx
function useRouter() {
  const { dispatch } = use(RouterContext)
  const navigate = useCallback((url) => {
    dispatch({ type: 'navigate', url })
  }, [dispatch])
  return { navigate }
}
```

## Custom Hooks Share Logic, Not State

Each call gets its own state instance. Two components calling `useOnlineStatus()` get separate state. To share state, use lifting or context.

## Hooks Compose by Chaining

The output of one hook feeds into another. Context-wrapping hooks are the canonical pattern:

```tsx
function useCounter(delay) {
  const [count, setCount] = useState(0)
  useInterval(() => setCount(c => c + 1), delay)
  return count
}

export function useTasks() { return use(TasksContext) }
export function useTasksDispatch() { return use(TasksDispatchContext) }
```

## Error Handling in Hooks

Return error state for expected, recoverable errors. Throw for unexpected errors (caught by Error Boundaries).

```tsx
function useFetch(url) {
  const [data, setData] = useState(null)
  const [error, setError] = useState(null)
  const [loading, setLoading] = useState(true)
  useEffect(() => {
    fetch(url).then(r => r.json()).then(setData).catch(setError).finally(() => setLoading(false))
  }, [url])
  return { data, error, loading }
}
```

## useEffectEvent for Callback Props

When a callback is used inside an effect, wrap it with `useEffectEvent` to remove it from dependencies.

```tsx
function useChatRoom({ serverUrl, roomId, onReceiveMessage }) {
  const onMessage = useEffectEvent(onReceiveMessage)
  useEffect(() => {
    const conn = createConnection({ serverUrl, roomId })
    conn.on('message', (msg) => onMessage(msg))
    return () => conn.disconnect()
  }, [roomId, serverUrl])  // onMessage not in deps
}
```

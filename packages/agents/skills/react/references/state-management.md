# State Management and Hooks

One Process: keep state at its owner, separate server state from client state, and extract hooks only when they share behavior.

## 1. Place state where it is used

### Colocate state by default

Place state as close to where it is used as possible. State at the top of the tree forces React to check every component on every update; colocated state lets React skip unaffected branches.

Template:
  ```text
  1. Used only by this component? Keep it here.
  2. Used only by one child? Move it down to that child.
  3. Used by siblings? Lift to the lowest common parent.
  4. Prop drilling painful? Use Context close to the consumers, not at root.
  ```

Push state back down during refactoring when lifted state is no longer shared.

### Lift state only to the lowest common parent

When siblings need shared state, lift it to their nearest common ancestor. Do not lift higher than necessary.

Never:
  ```tsx
  function App() {
    const [filter, setFilter] = useState('')
    return <Layout><Sidebar filter={filter} /><Content filter={filter} setFilter={setFilter} /></Layout>
  }
  ```

Example:
  ```tsx
  function FilteredContent() {
    const [filter, setFilter] = useState('')
    return <><FilterInput value={filter} onChange={setFilter} /><Results filter={filter} /></>
  }
  ```

## 2. Use Context for read-heavy shared data

### Split state and dispatch contexts

Context works well for read-heavy, write-rare data such as theme, auth, locale, and i18n. Separate state and dispatch contexts to prevent unnecessary re-renders.

Example:
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

## 3. Separate server state from client state

### Use a server-state library for server-owned data

Server state is data owned by the server and cached on the client. Use TanStack Query or SWR.

Example:
  ```tsx
  function UserProfile({ userId }) {
    const { data, isLoading, error } = useQuery({
      queryKey: ['user', userId],
      queryFn: () => fetchUser(userId),
    })
  }
  ```

### Use local state or a client store for browser-owned data

Client state is data owned by the browser or User Interface, such as modal open, selected tab, or form draft. Use `useState`, `useReducer`, or Zustand.

Example stack: TanStack Query for server state, `nuqs` for URL state, and `useState` or Zustand for client state.

### Use URL state for shareable state

Use `nuqs` for shareable filter, sort, and pagination state. Manual URL syncing creates bugs. Use URL state when the value should survive refresh, be bookmarkable, or be shareable through a link.

Example:
  ```tsx
  import { useQueryState } from 'nuqs'

  function SearchPage() {
    const [query, setQuery] = useQueryState('q')
    return <input value={query ?? ''} onChange={(event) => setQuery(event.target.value)} />
  }
  ```

## 4. Reach for external stores only for the right pressure

### Use Zustand or Jotai for fine-grained subscriptions

Context struggles with fine-grained subscriptions because every consumer re-renders when any part of the context value changes. Reach for Zustand or Jotai for fine-grained subscriptions, complex shared state with frequent updates, or state access outside React.

Example:
  ```tsx
  import { create } from 'zustand'

  const useStore = create((set) => ({
    count: 0,
    increment: () => set((state) => ({ count: state.count + 1 })),
  }))

  function Counter() {
    const count = useStore((state) => state.count)
  }
  ```

### Avoid global state anti-patterns

Most state is local User Interface state. Only auth, theme, and locale belong in global stores by default. Do not put rapidly changing values such as mouse position, scroll, or rapid typing into Context. Do not use global state as a server cache; use TanStack Query instead.

Never:
  ```tsx
  <AppContext.Provider value={{ user, theme, notifications, cart, locale }}>
  ```

Split providers or use an external store with selectors.

## 5. Choose form state by interaction

### Prefer uncontrolled forms in React 19

React auto-resets uncontrolled forms on success. Use `useFormStatus` for pending state without prop drilling.

Example:
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

### Use controlled forms when the User needs live feedback

Controlled forms fit real-time validation, conditional fields, and live preview. Use `react-hook-form` for complex controlled forms.

## 6. Name hooks after purpose

### Custom hook names start with use and a capital letter

The linter enforces hook names. Name hooks after purpose, not lifecycle. Functions that do not call hooks must not use the `use` prefix because regular functions can be called conditionally.

Never:
  ```tsx
  function useMount(fn) { useEffect(() => { fn() }, []) }
  function useSorted(items) { return items.slice().sort() }
  ```

Example:
  ```tsx
  function useChatRoom({ serverUrl, roomId }) { /* ... */ }
  function getSorted(items) { return items.slice().sort() }
  ```

## 7. Extract hooks only when they share behavior

### Extract a hook for duplicated or external-system logic

Extract when logic is duplicated across components, synchronizes with an external system, or when a complex Effect would benefit from clearer intent. Do not extract one or two lines or logic that does not call hooks.

Example:
  ```tsx
  useEffect(() => {
    const conn = createConnection({ serverUrl, roomId })
    conn.connect()
    return () => conn.disconnect()
  }, [roomId, serverUrl])
  ```

Example:
  ```tsx
  useChatRoom({ roomId, serverUrl })
  ```

### Custom hooks share logic, not state

Each hook call gets its own state instance. To share state, lift it or use Context.

## 8. Return hook values predictably

### Match return shape to consumer need

Use a single value for one value, a tuple for `useState`-like patterns, and an object for three or more values, partial destructuring, or growable APIs.

Example: `const isOnline = useOnlineStatus()`.
Example: `const [selected, onNext] = useList(products)`.
Example: `const { value, onChange } = useFormInput('Mary')`.

### Wrap returned functions in useCallback

Consumers may pass returned functions as props to memoized children or use them as Effect dependencies. Without `useCallback`, every render creates new references.

Example:
  ```tsx
  function useRouter() {
    const { dispatch } = use(RouterContext)
    const navigate = useCallback((url) => {
      dispatch({ type: 'navigate', url })
    }, [dispatch])
    return { navigate }
  }
  ```

### Compose hooks by chaining

The output of one hook feeds another. Context-wrapping hooks are the canonical pattern.

Example:
  ```tsx
  function useCounter(delay) {
    const [count, setCount] = useState(0)
    useInterval(() => setCount(count => count + 1), delay)
    return count
  }

  export function useTasks() { return use(TasksContext) }
  export function useTasksDispatch() { return use(TasksDispatchContext) }
  ```

## 9. Handle hook errors at the right boundary

### Return expected errors and throw unexpected errors

Return error state for expected, recoverable errors. Throw unexpected errors so Error Boundaries catch them.

Example:
  ```tsx
  function useFetch(url) {
    const [data, setData] = useState(null)
    const [error, setError] = useState(null)
    const [loading, setLoading] = useState(true)
    useEffect(() => {
      fetch(url).then(response => response.json()).then(setData).catch(setError).finally(() => setLoading(false))
    }, [url])
    return { data, error, loading }
  }
  ```

### Use useEffectEvent for callback props in Effects

When a callback is used inside an Effect, wrap it with `useEffectEvent` to remove it from dependencies.

Example:
  ```tsx
  function useChatRoom({ serverUrl, roomId, onReceiveMessage }) {
    const onMessage = useEffectEvent(onReceiveMessage)
    useEffect(() => {
      const conn = createConnection({ serverUrl, roomId })
      conn.on('message', (message) => onMessage(message))
      return () => conn.disconnect()
    }, [roomId, serverUrl])
  }
  ```

# Performance

One Process: remove unnecessary renders first, reduce shipped code second, and eliminate waterfalls last.

## 1. Remove unnecessary render work

### Derive state during render

If a value can be computed from props or state, compute it inline. Never store derived values in state or sync them through `useEffect`.

Never:
  ```tsx
  const [fullName, setFullName] = useState('')
  useEffect(() => {
    setFullName(firstName + ' ' + lastName)
  }, [firstName, lastName])
  ```

Example:
  ```tsx
  const fullName = firstName + ' ' + lastName
  ```

### Never define components inside components

Defining a component inside another creates a new type every render, causing full remounts that destroy state, DOM, and effects. Symptoms: inputs lose focus, animations restart, and effects re-run.

Never:
  ```tsx
  function UserProfile({ user, theme }) {
    const Avatar = () => <img src={user.avatarUrl} className={theme} />
    return <Avatar />
  }
  ```

Example:
  ```tsx
  function Avatar({ src, theme }: { src: string; theme: string }) {
    return <img src={src} className={theme} />
  }

  function UserProfile({ user, theme }) {
    return <Avatar src={user.avatarUrl} theme={theme} />
  }
  ```

### Hoist default non-primitive props

Default values such as `onClick = () => {}` break memoization. Extract them to module-level constants.

Example:
  ```tsx
  const NOOP = () => {}

  const Button = memo(function Button({ onClick = NOOP }: { onClick?: () => void }) {
    return <button onClick={onClick}>Click</button>
  })
  ```

### Use memo selectively

Wrap components only when re-rendering is expensive and props are referentially stable. React Compiler auto-memoizes when enabled.

### Skip useMemo for simple expressions

`useMemo` overhead exceeds simple expressions. Reserve `useMemo` for expensive computations such as filtering large arrays or building data structures.

Never:
  ```tsx
  const isLoading = useMemo(() => user.isLoading || posts.isLoading, [user.isLoading, posts.isLoading])
  ```

Example:
  ```tsx
  const isLoading = user.isLoading || posts.isLoading
  ```

### Narrow Effect dependencies

Use primitive values to minimize re-runs. Compute derived booleans outside the Effect.

Never:
  ```tsx
  useEffect(() => { console.log(user.id) }, [user])
  ```

Example:
  ```tsx
  useEffect(() => { console.log(user.id) }, [user.id])
  ```

### Use functional setState

Functional updates prevent stale closures and eliminate dependencies from `useCallback`.

Never:
  ```tsx
  const addItem = useCallback((item: Item) => {
    setItems([...items, item])
  }, [items])
  ```

Example:
  ```tsx
  const addItem = useCallback((item: Item) => {
    setItems(curr => [...curr, item])
  }, [])
  ```

### Use useDeferredValue for expensive derived renders

Example:
  ```tsx
  const [query, setQuery] = useState('')
  const deferredQuery = useDeferredValue(query)
  const filtered = useMemo(
    () => items.filter(item => fuzzyMatch(item, deferredQuery)),
    [items, deferredQuery]
  )
  const isStale = query !== deferredQuery
  ```

### Lazily initialize expensive state

Without the function form, the initializer runs on every render.

Never:
  ```tsx
  const [index, setIndex] = useState(buildSearchIndex(items))
  ```

Example:
  ```tsx
  const [index, setIndex] = useState(() => buildSearchIndex(items))
  ```

### Use useTransition for non-urgent updates

Mark frequent, non-urgent state updates as transitions to keep the User Interface responsive.

Example:
  ```tsx
  import { startTransition } from 'react'

  const handler = () => {
    startTransition(() => setScrollY(window.scrollY))
  }
  ```

### Use refs for transient values

Store frequently changing values that do not need re-renders, such as mouse position and timers, in refs. Update DOM directly when the value is purely visual.

Example:
  ```tsx
  const lastXRef = useRef(0)
  const dotRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const onMove = (event: MouseEvent) => {
      lastXRef.current = event.clientX
      if (dotRef.current) dotRef.current.style.transform = `translateX(${event.clientX}px)`
    }
    window.addEventListener('mousemove', onMove)
    return () => window.removeEventListener('mousemove', onMove)
  }, [])
  ```

## 2. Reduce bundle size

### Avoid barrel file imports

Import directly from source files. Barrel files cost 200 to 800 milliseconds per import. Tree-shaking does not help when libraries are external.

Never:
  ```tsx
  import { Check, X, Menu } from 'lucide-react'
  import { Button, TextField } from '@mui/material'
  ```

Measured: `lucide-react` barrel import loaded 1,583 modules in about 2.8 seconds; `@mui/material` barrel import loaded 2,225 modules in about 4.2 seconds.

Example for Next.js 13.5 and later with `optimizePackageImports`:
  ```tsx
  import { Check, X, Menu } from 'lucide-react'
  ```

Example outside Next.js:
  ```tsx
  import Button from '@mui/material/Button'
  import TextField from '@mui/material/TextField'
  ```

Commonly affected: `lucide-react`, `@mui/material`, `@mui/icons-material`, `@tabler/icons-react`, `react-icons`, `@radix-ui/react-*`, `lodash`, and `date-fns`.

### Dynamically import heavy components

Example:
  ```tsx
  import dynamic from 'next/dynamic'

  const MonacoEditor = dynamic(
    () => import('./monaco-editor').then(module => module.MonacoEditor),
    { ssr: false }
  )
  ```

### Preload on hover and focus

Example:
  ```tsx
  function EditorButton({ onClick }: { onClick: () => void }) {
    const preload = () => { void import('./monaco-editor') }
    return (
      <button onMouseEnter={preload} onFocus={preload} onClick={onClick}>
        Open Editor
      </button>
    )
  }
  ```

### Defer non-critical libraries

Analytics, logging, and error tracking load after hydration with `dynamic(() => import(...), { ssr: false })`.

### Load large modules only when activated

Example:
  ```tsx
  function AnimationPlayer({ enabled }: { enabled: boolean }) {
    const [frames, setFrames] = useState<Frame[] | null>(null)

    useEffect(() => {
      if (enabled && !frames && typeof window !== 'undefined') {
        import('./animation-frames.js')
          .then(module => setFrames(module.frames))
          .catch(() => {})
      }
    }, [enabled, frames])

    if (!frames) return <Skeleton />
    return <Canvas frames={frames} />
  }
  ```

## 3. Eliminate waterfalls

### Use Promise.all for independent operations

Never:
  ```tsx
  const user = await fetchUser()
  const posts = await fetchPosts()
  const comments = await fetchComments()
  ```

Example:
  ```tsx
  const [user, posts, comments] = await Promise.all([
    fetchUser(), fetchPosts(), fetchComments()
  ])
  ```

### Start promises before awaiting

Example:
  ```tsx
  export async function GET(request: Request) {
    const sessionPromise = auth()
    const configPromise = fetchConfig()
    const session = await sessionPromise
    const [config, data] = await Promise.all([
      configPromise, fetchData(session.user.id)
    ])
    return Response.json({ data, config })
  }
  ```

### Defer await until needed

Never:
  ```tsx
  async function handleRequest(userId: string, skip: boolean) {
    const userData = await fetchUserData(userId)
    if (skip) return { skipped: true }
    return processUserData(userData)
  }
  ```

Example:
  ```tsx
  async function handleRequest(userId: string, skip: boolean) {
    if (skip) return { skipped: true }
    const userData = await fetchUserData(userId)
    return processUserData(userData)
  }
  ```

### Chain nested fetches per item

Chain dependent fetches within each item's promise so a slow item does not block the rest.

Never:
  ```tsx
  const chats = await Promise.all(chatIds.map(id => getChat(id)))
  const authors = await Promise.all(chats.map(chat => getUser(chat.author)))
  ```

Example:
  ```tsx
  const authors = await Promise.all(
    chatIds.map(id => getChat(id).then(chat => getUser(chat.author)))
  )
  ```

### Use strategic Suspense boundaries

Stream content progressively. The shell renders immediately and data streams in.

Example:
  ```tsx
  function Page() {
    return (
      <div>
        <Header />
        <Suspense fallback={<Skeleton />}>
          <DataDisplay />
        </Suspense>
        <Footer />
      </div>
    )
  }
  ```

### Share promises across components

Example:
  ```tsx
  function Page() {
    const dataPromise = fetchData()
    return (
      <Suspense fallback={<Skeleton />}>
        <DataDisplay dataPromise={dataPromise} />
        <DataSummary dataPromise={dataPromise} />
      </Suspense>
    )
  }

  function DataDisplay({ dataPromise }: { dataPromise: Promise<Data> }) {
    const data = use(dataPromise)
    return <div>{data.content}</div>
  }
  ```

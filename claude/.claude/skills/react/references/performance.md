# Performance

## Re-render Optimization

### Derive State During Render

If a value can be computed from props or state, compute it inline. Never store derived values in state or sync them via useEffect.

**Incorrect:**
```tsx
const [fullName, setFullName] = useState('')
useEffect(() => {
  setFullName(firstName + ' ' + lastName)
}, [firstName, lastName])
```

**Correct:**
```tsx
const fullName = firstName + ' ' + lastName
```

### Never Define Components Inside Components

Defining a component inside another creates a new type every render, causing full remounts — destroying state, DOM, and effects. Symptoms: inputs lose focus, animations restart, effects re-run.

**Incorrect:**
```tsx
function UserProfile({ user, theme }) {
  const Avatar = () => <img src={user.avatarUrl} className={theme} />
  return <Avatar />
}
```

**Correct:**
```tsx
function Avatar({ src, theme }: { src: string; theme: string }) {
  return <img src={src} className={theme} />
}

function UserProfile({ user, theme }) {
  return <Avatar src={user.avatarUrl} theme={theme} />
}
```

### Hoist Default Non-Primitive Props

Default values like `onClick = () => {}` break memoization. Extract to module-level constants.

```tsx
const NOOP = () => {}

const Button = memo(function Button({ onClick = NOOP }: { onClick?: () => void }) {
  return <button onClick={onClick}>Click</button>
})
```

### Use memo() Selectively

Only wrap components when re-rendering is expensive AND props are referentially stable. React Compiler auto-memoizes when enabled.

### Skip useMemo for Simple Expressions

```tsx
// Incorrect — useMemo overhead exceeds the expression
const isLoading = useMemo(() => user.isLoading || posts.isLoading, [user.isLoading, posts.isLoading])

// Correct
const isLoading = user.isLoading || posts.isLoading
```

Reserve `useMemo` for expensive computations: filtering large arrays, building data structures.

### Narrow Effect Dependencies

Use primitive values to minimize re-runs. Compute derived booleans outside the effect.

```tsx
// Incorrect
useEffect(() => { console.log(user.id) }, [user])

// Correct
useEffect(() => { console.log(user.id) }, [user.id])
```

### Functional setState

Prevents stale closures and eliminates dependencies from useCallback.

```tsx
// Incorrect — stale closure, depends on items
const addItem = useCallback((item: Item) => {
  setItems([...items, item])
}, [items])

// Correct — functional update, no deps needed
const addItem = useCallback((item: Item) => {
  setItems(curr => [...curr, item])
}, [])
```

### useDeferredValue for Expensive Derived Renders

```tsx
const [query, setQuery] = useState('')
const deferredQuery = useDeferredValue(query)
const filtered = useMemo(
  () => items.filter(item => fuzzyMatch(item, deferredQuery)),
  [items, deferredQuery]
)
const isStale = query !== deferredQuery
```

### Lazy State Initialization

Pass a function to `useState` for expensive initial values. Without the function form, the initializer runs on every render.

```tsx
// Incorrect — buildSearchIndex runs every render
const [index, setIndex] = useState(buildSearchIndex(items))

// Correct — runs only once
const [index, setIndex] = useState(() => buildSearchIndex(items))
```

### useTransition for Non-Urgent Updates

Mark frequent, non-urgent state updates as transitions to keep the UI responsive.

```tsx
import { startTransition } from 'react'

const handler = () => {
  startTransition(() => setScrollY(window.scrollY))
}
```

### useRef for Transient Values

Store frequently changing values that don't need re-renders (mouse position, timers) in refs. Update DOM directly.

```tsx
const lastXRef = useRef(0)
const dotRef = useRef<HTMLDivElement>(null)

useEffect(() => {
  const onMove = (e: MouseEvent) => {
    lastXRef.current = e.clientX
    if (dotRef.current) dotRef.current.style.transform = `translateX(${e.clientX}px)`
  }
  window.addEventListener('mousemove', onMove)
  return () => window.removeEventListener('mousemove', onMove)
}, [])
```

## Bundle Size

### Avoid Barrel File Imports

Import directly from source files. Barrel files cost 200-800ms per import. Tree-shaking doesn't help when libraries are external.

**Incorrect:**
```tsx
import { Check, X, Menu } from 'lucide-react'     // 1,583 modules, ~2.8s
import { Button, TextField } from '@mui/material'  // 2,225 modules, ~4.2s
```

**Correct (Next.js 13.5+ with optimizePackageImports):**
```tsx
import { Check, X, Menu } from 'lucide-react'  // Auto-transformed
```

**Correct (non-Next.js):**
```tsx
import Button from '@mui/material/Button'
import TextField from '@mui/material/TextField'
```

Commonly affected: `lucide-react`, `@mui/material`, `@mui/icons-material`, `@tabler/icons-react`, `react-icons`, `@radix-ui/react-*`, `lodash`, `date-fns`.

### Dynamic Imports for Heavy Components

```tsx
import dynamic from 'next/dynamic'

const MonacoEditor = dynamic(
  () => import('./monaco-editor').then(m => m.MonacoEditor),
  { ssr: false }
)
```

### Preload on Hover

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

### Defer Non-Critical Libraries

Analytics, logging, error tracking — load after hydration with `dynamic(() => import(...), { ssr: false })`.

### Conditional Module Loading

Load large data or modules only when a feature is activated.

```tsx
function AnimationPlayer({ enabled }: { enabled: boolean }) {
  const [frames, setFrames] = useState<Frame[] | null>(null)

  useEffect(() => {
    if (enabled && !frames && typeof window !== 'undefined') {
      import('./animation-frames.js')
        .then(mod => setFrames(mod.frames))
        .catch(() => {})
    }
  }, [enabled, frames])

  if (!frames) return <Skeleton />
  return <Canvas frames={frames} />
}
```

## Eliminating Waterfalls

### Promise.all for Independent Operations

**Incorrect (3 sequential round trips):**
```tsx
const user = await fetchUser()
const posts = await fetchPosts()
const comments = await fetchComments()
```

**Correct (1 round trip):**
```tsx
const [user, posts, comments] = await Promise.all([
  fetchUser(), fetchPosts(), fetchComments()
])
```

### Start Promises Before Awaiting

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


### Defer Await Until Needed

Move `await` into the branch where the result is actually used.

```tsx
// Incorrect — blocks both branches
async function handleRequest(userId: string, skip: boolean) {
  const userData = await fetchUserData(userId)
  if (skip) return { skipped: true }
  return processUserData(userData)
}

// Correct — only blocks when needed
async function handleRequest(userId: string, skip: boolean) {
  if (skip) return { skipped: true }
  const userData = await fetchUserData(userId)
  return processUserData(userData)
}
```

### Chain Nested Fetches Per Item

Chain dependent fetches within each item's promise so a slow item doesn't block the rest.

```tsx
// Incorrect — one slow chat blocks all author fetches
const chats = await Promise.all(chatIds.map(id => getChat(id)))
const authors = await Promise.all(chats.map(chat => getUser(chat.author)))

// Correct — each item chains independently
const authors = await Promise.all(
  chatIds.map(id => getChat(id).then(chat => getUser(chat.author)))
)
```

### Strategic Suspense Boundaries

Stream content progressively. The shell renders immediately, data streams in.

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

### Share Promises Across Components

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

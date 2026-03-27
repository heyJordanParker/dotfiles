# Data Fetching Patterns

## TanStack Query vs SWR

Choose SWR for read-heavy apps with simple mutations, when bundle size matters (4.2KB vs 11.4KB), or when simplicity is priority. Choose TanStack Query for complex mutations with optimistic updates, fine-grained cache control (staleTime, gcTime), hierarchical query key invalidation, built-in DevTools, or infinite scroll.

## Query Key Factory Pattern

Organize keys hierarchically for targeted invalidation. Keys must be arrays and are serialized via JSON.

```tsx
const todoKeys = {
  all: ['todos'] as const,
  lists: () => [...todoKeys.all, 'list'] as const,
  list: (filters: Filters) => [...todoKeys.lists(), filters] as const,
  details: () => [...todoKeys.all, 'detail'] as const,
  detail: (id: number) => [...todoKeys.details(), id] as const,
}

// Usage
useQuery({ queryKey: todoKeys.detail(5), queryFn: () => fetchTodo(5) })

// Invalidation — all todo lists
queryClient.invalidateQueries({ queryKey: todoKeys.lists() })
```

## Query Functions Must Throw on Errors

TanStack Query does not treat non-2xx status codes as errors. You must throw explicitly.

**Incorrect:**
```tsx
const queryFn = () => fetch('/api/todos').then(r => r.json())
```

**Correct:**
```tsx
async function fetchTodos(): Promise<Todo[]> {
  const res = await fetch('/api/todos')
  if (!res.ok) throw new Error(`Failed: ${res.status}`)
  return res.json()
}
```

## Mutations with Invalidation

Invalidate related queries on mutation success to trigger refetch. Return the Promise from invalidation so the mutation stays pending until the refetch completes.

```tsx
const queryClient = useQueryClient()

const mutation = useMutation({
  mutationFn: (newTodo: { title: string }) =>
    fetch('/api/todos', { method: 'POST', body: JSON.stringify(newTodo) })
      .then(res => { if (!res.ok) throw new Error('Failed'); return res.json() }),
  onSuccess: () => queryClient.invalidateQueries({ queryKey: ['todos'] }),
})
```

## Optimistic Updates

**Approach 1 — UI variables (simpler, one display location):**
```tsx
const { isPending, variables, mutate, isError } = useMutation({
  mutationFn: (text: string) => axios.post('/api/todos', { text }),
  onSettled: () => queryClient.invalidateQueries({ queryKey: ['todos'] }),
})

// In JSX — show pending item with reduced opacity
{isPending && <li style={{ opacity: 0.5 }}>{variables}</li>}
{isError && <li style={{ color: 'red' }}>{variables} <button onClick={() => mutate(variables)}>Retry</button></li>}
```

**Approach 2 — cache manipulation (multiple display locations, rollback):**
```tsx
useMutation({
  mutationFn: updateTodo,
  onMutate: async (newTodo) => {
    await queryClient.cancelQueries({ queryKey: ['todos'] })
    const previous = queryClient.getQueryData(['todos'])
    queryClient.setQueryData(['todos'], (old) => [...old, newTodo])
    return { previous }
  },
  onError: (err, newTodo, context) => {
    queryClient.setQueryData(['todos'], context.previous)
  },
  onSettled: () => {
    queryClient.invalidateQueries({ queryKey: ['todos'] })
  },
})
```

## Prefetch on Hover

```tsx
function TodoLink({ id }: { id: number }) {
  const queryClient = useQueryClient()
  const prefetch = () => {
    queryClient.prefetchQuery({
      queryKey: ['todo', id],
      queryFn: () => fetchTodo(id),
      staleTime: 5 * 60 * 1000,
    })
  }
  return <Link to={`/todos/${id}`} onMouseEnter={prefetch} onFocus={prefetch}>Todo {id}</Link>
}
```

## useSuspenseQuery with Error Boundaries

Data is guaranteed defined (no `T | undefined`). Wrap with Suspense for loading and QueryErrorResetBoundary for error recovery.

```tsx
import { QueryErrorResetBoundary, useSuspenseQuery } from '@tanstack/react-query'
import { ErrorBoundary } from 'react-error-boundary'

function App() {
  return (
    <QueryErrorResetBoundary>
      {({ reset }) => (
        <ErrorBoundary onReset={reset} fallbackRender={({ resetErrorBoundary }) => (
          <div>Error! <button onClick={resetErrorBoundary}>Retry</button></div>
        )}>
          <Suspense fallback={<Skeleton />}>
            <TodoList />
          </Suspense>
        </ErrorBoundary>
      )}
    </QueryErrorResetBoundary>
  )
}

function TodoList() {
  const { data } = useSuspenseQuery({ queryKey: ['todos'], queryFn: fetchTodos })
  return <ul>{data.map(t => <li key={t.id}>{t.title}</li>)}</ul>
}
```

## Placeholder and Initial Data

`placeholderData` is NOT cached — shown while real data loads. `initialData` IS cached — treated as real data.

```tsx
// Keep previous data during key transition
useQuery({
  queryKey: ['todo', id],
  queryFn: () => fetchTodo(id),
  placeholderData: (previousData) => previousData,
})

// Seed from another query's cache
useQuery({
  queryKey: ['blogPost', blogPostId],
  queryFn: () => fetchBlogPost(blogPostId),
  placeholderData: () => queryClient.getQueryData(['blogPosts'])?.find(d => d.id === blogPostId),
})
```

## Dependent Queries

Use `enabled` to defer a query until its dependency is ready.

```tsx
const { data: user } = useQuery({ queryKey: ['user', email], queryFn: getUserByEmail })
const { data: projects } = useQuery({
  queryKey: ['projects', user?.id],
  queryFn: getProjectsByUser,
  enabled: !!user?.id,
})
```

Dependent queries create waterfalls. Prefer restructuring backend APIs to parallelize when possible.

## Infinite Queries

```tsx
const { data, fetchNextPage, hasNextPage, isFetchingNextPage } = useInfiniteQuery({
  queryKey: ['projects'],
  queryFn: ({ pageParam }) => fetch(`/api/projects?cursor=${pageParam}`).then(r => r.json()),
  initialPageParam: 0,
  getNextPageParam: (lastPage) => lastPage.nextCursor,
})
```

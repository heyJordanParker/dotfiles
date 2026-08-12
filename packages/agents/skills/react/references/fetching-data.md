# Data Fetching

One Process: choose the data-fetching library, make cache keys explicit, throw on failures, and verify loading, error, mutation, and cache behavior.

## 1. Choose the library by behavior

### Use SWR for simple read-heavy applications

Choose SWR when mutations are simple, bundle size matters, or simplicity outranks cache control. Measured bundle size: SWR is 4.2 kilobytes and TanStack Query is 11.4 kilobytes.

### Use TanStack Query for complex server state

Choose TanStack Query for complex mutations with optimistic updates, fine-grained cache control through `staleTime` and `gcTime`, hierarchical query-key invalidation, built-in DevTools, or infinite scroll.

## 2. Define query keys hierarchically

### Use a query key factory

Organize keys hierarchically for targeted invalidation. Query keys must be arrays and are serialized through JSON.

Example:
  ```tsx
  const todoKeys = {
    all: ['todos'] as const,
    lists: () => [...todoKeys.all, 'list'] as const,
    list: (filters: Filters) => [...todoKeys.lists(), filters] as const,
    details: () => [...todoKeys.all, 'detail'] as const,
    detail: (id: number) => [...todoKeys.details(), id] as const,
  }

  useQuery({ queryKey: todoKeys.detail(5), queryFn: () => fetchTodo(5) })
  queryClient.invalidateQueries({ queryKey: todoKeys.lists() })
  ```

## 3. Make failures explicit

### Query functions must throw on errors

TanStack Query does not treat non-2xx status codes as errors. Throw explicitly.

Never:
  ```tsx
  const queryFn = () => fetch('/api/todos').then(response => response.json())
  ```

Example:
  ```tsx
  async function fetchTodos(): Promise<Todo[]> {
    const response = await fetch('/api/todos')
    if (!response.ok) throw new Error(`Failed: ${response.status}`)
    return response.json()
  }
  ```

## 4. Keep mutations and cache invalidation together

### Return invalidation promises from onSuccess

Invalidate related queries on mutation success. Return the invalidation Promise so the mutation stays pending until refetch completes.

Example:
  ```tsx
  const queryClient = useQueryClient()

  const mutation = useMutation({
    mutationFn: (newTodo: { title: string }) =>
      fetch('/api/todos', { method: 'POST', body: JSON.stringify(newTodo) })
        .then(response => { if (!response.ok) throw new Error('Failed'); return response.json() }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['todos'] }),
  })
  ```

### Use optimistic User Interface variables for one display location

Example:
  ```tsx
  const { isPending, variables, mutate, isError } = useMutation({
    mutationFn: (text: string) => axios.post('/api/todos', { text }),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['todos'] }),
  })

  {isPending && <li style={{ opacity: 0.5 }}>{variables}</li>}
  {isError && <li style={{ color: 'red' }}>{variables} <button onClick={() => mutate(variables)}>Retry</button></li>}
  ```

### Use cache manipulation for multiple display locations or rollback

Example:
  ```tsx
  useMutation({
    mutationFn: updateTodo,
    onMutate: async (newTodo) => {
      await queryClient.cancelQueries({ queryKey: ['todos'] })
      const previous = queryClient.getQueryData(['todos'])
      queryClient.setQueryData(['todos'], (old) => [...old, newTodo])
      return { previous }
    },
    onError: (error, newTodo, context) => {
      queryClient.setQueryData(['todos'], context.previous)
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['todos'] })
    },
  })
  ```

## 5. Prefetch before the User needs the data

### Prefetch on hover and focus

Example:
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

## 6. Integrate loading and errors with React

### Use useSuspenseQuery with Error Boundaries

`useSuspenseQuery` guarantees data is defined. Wrap it with Suspense for loading and `QueryErrorResetBoundary` plus `ErrorBoundary` for recovery.

Example:
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
    return <ul>{data.map(todo => <li key={todo.id}>{todo.title}</li>)}</ul>
  }
  ```

## 7. Distinguish placeholder and initial data

### Use placeholderData for temporary display and initialData for cache seeding

`placeholderData` is not cached; it is shown while real data loads. `initialData` is cached and treated as real data.

Example:
  ```tsx
  useQuery({
    queryKey: ['todo', id],
    queryFn: () => fetchTodo(id),
    placeholderData: (previousData) => previousData,
  })

  useQuery({
    queryKey: ['blogPost', blogPostId],
    queryFn: () => fetchBlogPost(blogPostId),
    placeholderData: () => queryClient.getQueryData(['blogPosts'])?.find(post => post.id === blogPostId),
  })
  ```

## 8. Control dependent and infinite queries

### Gate dependent queries with enabled

Use `enabled` to defer a query until its dependency is ready. Dependent queries create waterfalls, so prefer restructuring backend APIs to parallelize when possible.

Example:
  ```tsx
  const { data: user } = useQuery({ queryKey: ['user', email], queryFn: getUserByEmail })
  const { data: projects } = useQuery({
    queryKey: ['projects', user?.id],
    queryFn: getProjectsByUser,
    enabled: !!user?.id,
  })
  ```

### Use useInfiniteQuery for cursor pages

Example:
  ```tsx
  const { data, fetchNextPage, hasNextPage, isFetchingNextPage } = useInfiniteQuery({
    queryKey: ['projects'],
    queryFn: ({ pageParam }) => fetch(`/api/projects?cursor=${pageParam}`).then(response => response.json()),
    initialPageParam: 0,
    getNextPageParam: (lastPage) => lastPage.nextCursor,
  })
  ```

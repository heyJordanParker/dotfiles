# Error Handling

One Process: use `react-error-boundary`, place boundaries around the right sections, reset them deliberately, and handle errors they cannot catch at the call site.

## 1. Use the library boundary

### Use react-error-boundary

The official React docs recommend `react-error-boundary`, maintained by React core team member Brian Vaughn. There is no function-component equivalent for error boundaries.

Never:
  ```tsx
  class MyErrorBoundary extends React.Component {
    state = { hasError: false };
    static getDerivedStateFromError() { return { hasError: true }; }
    render() {
      if (this.state.hasError) return <p>Error</p>;
      return this.props.children;
    }
  }
  ```

Example:
  ```tsx
  import { ErrorBoundary } from "react-error-boundary";

  <ErrorBoundary
    fallbackRender={({ error, resetErrorBoundary }) => (
      <div role="alert">
        <p>Something went wrong: {error.message}</p>
        <button onClick={resetErrorBoundary}>Try again</button>
      </div>
    )}
    onError={(error, info) => logToService(error, info)}
    onReset={() => { /* cleanup before retry */ }}
  >
    <App />
  </ErrorBoundary>
  ```

## 2. Put ErrorBoundary outside Suspense

### ErrorBoundary wraps Suspense

ErrorBoundary catches loading failures and render errors. It must be the outer wrapper so rejected promises from `use()` and failed lazy loads are caught.

Never:
  ```tsx
  <Suspense fallback={<Spinner />}>
    <ErrorBoundary fallback={<Error />}>
      <AsyncComponent />
    </ErrorBoundary>
  </Suspense>
  ```

Example:
  ```tsx
  <ErrorBoundary fallback={<p>Failed to load</p>}>
    <Suspense fallback={<Spinner />}>
      <AsyncComponent />
    </Suspense>
  </ErrorBoundary>
  ```

## 3. Place boundaries by User capability

### Use granular per-section boundaries

A single page-level boundary loses all page state on error. Per-section boundaries isolate failures so the rest of the page remains functional. Always wrap Critical Path sections such as checkout and payment independently.

Never:
  ```tsx
  <ErrorBoundary fallback={<PageError />}>
    <Sidebar />
    <Feed />
    <Comments />
  </ErrorBoundary>
  ```

Example:
  ```tsx
  <Layout>
    <ErrorBoundary fallback={<SidebarError />}>
      <Sidebar />
    </ErrorBoundary>
    <Main>
      <ErrorBoundary fallback={<FeedError />}>
        <Feed />
      </ErrorBoundary>
      <ErrorBoundary fallback={<CommentsError />}>
        <Comments />
      </ErrorBoundary>
    </Main>
  </Layout>
  ```

Use an outer catch-all boundary with a full-page fallback and inner per-widget boundaries with retry buttons.

## 4. Reset boundaries when dependencies change

### Use resetKeys for automatic retry

When underlying data changes, the error boundary should retry. `resetKeys` resets when any value in the array changes.

Never:
  ```tsx
  <ErrorBoundary fallback={<Error />}>
    <UserProfile userId={userId} />
  </ErrorBoundary>
  ```

Example:
  ```tsx
  <ErrorBoundary
    resetKeys={[userId]}
    fallbackRender={({ error, resetErrorBoundary }) => (
      <div>
        <p>Failed to load profile: {error.message}</p>
        <button onClick={resetErrorBoundary}>Retry</button>
      </div>
    )}
  >
    <UserProfile userId={userId} />
  </ErrorBoundary>
  ```

## 5. Let React surface render-time async errors

### Transition errors surface through Error Boundaries

Errors thrown inside `startTransition` are caught by the nearest Error Boundary. This integrates with React 19 Actions.

Example:
  ```tsx
  function AddCommentSection() {
    return (
      <ErrorBoundary fallback={<p>Failed to add comment</p>}>
        <AddCommentButton />
      </ErrorBoundary>
    );
  }

  function AddCommentButton() {
    const [pending, startTransition] = useTransition();
    return (
      <button
        disabled={pending}
        onClick={() => {
          startTransition(() => {
            addComment();
          });
        }}
      >
        {pending ? "Adding..." : "Add comment"}
      </button>
    );
  }
  ```

### Rejected promises through use are caught by Error Boundaries

The `use()` hook integrates with Suspense for loading and ErrorBoundary for errors. No try/catch is needed around the render.

Example:
  ```tsx
  function MessageContainer({ messagePromise }) {
    return (
      <ErrorBoundary fallback={<p>Failed to load message</p>}>
        <Suspense fallback={<p>Loading...</p>}>
          <Message messagePromise={messagePromise} />
        </Suspense>
      </ErrorBoundary>
    );
  }

  function Message({ messagePromise }) {
    const content = use(messagePromise);
    return <p>{content}</p>;
  }
  ```

## 6. Handle errors boundaries cannot catch

### Know the boundary limit

Error boundaries catch errors during rendering, lifecycle methods, constructors, React Router loaders and actions, rejected promises through `use()`, and `startTransition` callbacks. They do not catch event handlers, async code, or server-side rendering errors.

Example:
  ```tsx
  function Component() {
    function handleClick() {
      try {
        riskyOperation();
      } catch (error) {
        setError(error.message);
      }
    }

    useEffect(() => {
      fetchData().catch(error => setError(error.message));
    }, []);

    return <button onClick={handleClick}>Click</button>;
  }
  ```

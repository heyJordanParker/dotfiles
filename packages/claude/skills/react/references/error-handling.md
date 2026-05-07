# Error Boundaries and Recovery

## Use react-error-boundary Library

The official React docs recommend react-error-boundary (by React core team member Brian Vaughn). There is no function component equivalent for error boundaries — this library provides the cleanest API.

```tsx
// Incorrect: Writing your own class-based error boundary
class MyErrorBoundary extends React.Component {
  state = { hasError: false };
  static getDerivedStateFromError() { return { hasError: true }; }
  render() {
    if (this.state.hasError) return <p>Error</p>;
    return this.props.children;
  }
}

// Correct: Use react-error-boundary
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

## ErrorBoundary Wraps Suspense, Not Inside

ErrorBoundary catches both loading failures and render errors. It must be the outer wrapper so rejected promises from `use()` and failed lazy loads are caught.

```tsx
// Incorrect: ErrorBoundary inside Suspense
<Suspense fallback={<Spinner />}>
  <ErrorBoundary fallback={<Error />}>
    <AsyncComponent />
  </ErrorBoundary>
</Suspense>

// Correct: ErrorBoundary wraps Suspense
<ErrorBoundary fallback={<p>Failed to load</p>}>
  <Suspense fallback={<Spinner />}>
    <AsyncComponent />
  </Suspense>
</ErrorBoundary>
```

## Use Granular Per-Section Boundaries

A single page-level boundary loses all page state on error. Per-section boundaries isolate failures so the rest of the page remains functional.

```tsx
// Incorrect: One boundary for the entire page
<ErrorBoundary fallback={<PageError />}>
  <Sidebar />
  <Feed />
  <Comments />
</ErrorBoundary>

// Correct: Each section has its own boundary
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

Layer boundaries: outer catch-all with full-page fallback, inner per-widget boundaries with retry buttons. Always wrap critical sections (checkout, payment) independently.

## Use resetKeys for Auto-Reset on Dependency Change

When the underlying data changes, the error boundary should automatically retry. resetKeys triggers a reset when any value in the array changes.

```tsx
// Incorrect: Error stays visible even after user changes
<ErrorBoundary fallback={<Error />}>
  <UserProfile userId={userId} />
</ErrorBoundary>

// Correct: Boundary resets when userId changes
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

## Transition Errors Surface Through Error Boundaries

Errors thrown inside startTransition are caught by the nearest error boundary. This integrates with React 19 Actions.

```tsx
// Correct: Transition errors caught by ErrorBoundary
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
          addComment(); // If this throws, ErrorBoundary catches it
        });
      }}
    >
      {pending ? "Adding..." : "Add comment"}
    </button>
  );
}
```

## Rejected Promises via use() Are Caught by Error Boundaries

The `use()` hook integrates with Suspense for loading and ErrorBoundary for errors. No try/catch needed.

```tsx
// Correct: use() + Suspense + ErrorBoundary = complete async handling
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
  const content = use(messagePromise); // Suspends or throws
  return <p>{content}</p>;
}
```

## Know What Error Boundaries Do Not Catch

Error boundaries only catch errors during rendering, lifecycle methods, and constructors. They do not catch event handlers, async code, or SSR errors.

```tsx
// Error boundaries DO NOT catch these:
function Component() {
  // Event handlers — use try/catch
  function handleClick() {
    try {
      riskyOperation();
    } catch (e) {
      setError(e.message);
    }
  }

  // Async code in effects — use try/catch
  useEffect(() => {
    fetchData().catch(e => setError(e.message));
  }, []);

  return <button onClick={handleClick}>Click</button>;
}

// Error boundaries DO catch these:
// - Errors thrown during render
// - Errors in loaders/actions (React Router)
// - Rejected promises via use()
// - Errors in startTransition callbacks
// - Errors in componentDidMount/componentDidUpdate
```


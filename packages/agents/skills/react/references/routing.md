# React Router Patterns

## Use createBrowserRouter for Data-Aware Routing

createBrowserRouter enables loaders, actions, pending states, and error handling. It replaces `<BrowserRouter>` for applications that need data loading. Use Framework Mode for file-based routing with automatic code splitting.

```tsx
// Incorrect: Declarative routes without data loading
<BrowserRouter>
  <Routes>
    <Route path="/" element={<Root />}>
      <Route path="dashboard" element={<Dashboard />} />
    </Route>
  </Routes>
</BrowserRouter>

// Correct: Data router with loaders and error handling
const router = createBrowserRouter([
  {
    path: "/",
    Component: Root,
    errorElement: <RootError />,
    children: [
      { index: true, Component: Home },
      {
        path: "dashboard",
        Component: Dashboard,
        loader: () => fetchStats(),
        errorElement: <DashboardError />,
      },
    ],
  },
]);

createRoot(root).render(<RouterProvider router={router} />);
```

## Loaders for Reads, Actions for Writes

Loaders fetch data before the route renders. Actions handle form mutations. After an action completes, React Router automatically revalidates all loaders on the page.

```tsx
// Correct: Loader fetches, action mutates, auto-revalidation keeps UI in sync
createBrowserRouter([{
  path: "/todos",
  loader: async () => db.todos.findAll(),
  action: async ({ request }) => {
    const formData = await request.formData();
    await db.todos.create({ title: formData.get("title") });
    return { ok: true }; // All loaders on the page revalidate automatically
  },
  Component: Todos,
}]);

function Todos({ loaderData }) {
  return (
    <div>
      <ul>{loaderData.map(t => <li key={t.id}>{t.title}</li>)}</ul>
      <Form method="post">
        <input name="title" />
        <button type="submit">Add</button>
      </Form>
    </div>
  );
}
```

## Use useFetcher for Inline Mutations

useFetcher submits data without navigating or creating history entries. Use it for toggles, inline edits, and background operations.

```tsx
// Incorrect: Form causes full page navigation for a simple toggle
<Form method="post" action={`/todos/${todo.id}/toggle`}>
  <button type="submit">Toggle</button>
</Form>

// Correct: useFetcher mutates without navigation
function TodoItem({ todo }) {
  const fetcher = useFetcher();

  // Optimistic UI: read intended value from form data
  let isDone = todo.done;
  if (fetcher.formData) {
    isDone = fetcher.formData.get("done") === "true";
  }

  return (
    <fetcher.Form method="post" action={`/todos/${todo.id}`}>
      <input type="hidden" name="done" value={String(!isDone)} />
      <button type="submit">{isDone ? "Undo" : "Done"}</button>
    </fetcher.Form>
  );
}
```

## Lazy Load at Route Boundaries

Use the `lazy` property to code-split routes. Keep the shell (root layout, navigation) eagerly loaded. Lazy loads both component and loader before rendering — no nested spinners.

```tsx
// Correct: Shell is eager, feature routes are lazy
createBrowserRouter([
  {
    path: "/",
    Component: Layout,   // Always loaded
    children: [
      { index: true, Component: Home },  // Landing page — eager
      { path: "about", lazy: () => import("./about") },
      { path: "dashboard", lazy: () => import("./dashboard") },
      {
        path: "reports/:id",
        lazy: async () => {
          const [Component, loader] = await Promise.all([
            import("./report"),
            import("./report-loader"),
          ]);
          return { Component, loader };
        },
      },
    ],
  },
]);
```

## Protect Routes with Middleware or Loader Redirect

Use middleware for cross-cutting authentication. Use loader redirects for route-specific authorization. Throw redirect() to stop execution immediately.

```tsx
// Correct: Middleware for auth, context passes user to loaders
async function authMiddleware({ context }) {
  const user = await getUser();
  if (!user) throw redirect("/login");
  context.set(userContext, user);
}

const routes = [{
  path: "/",
  middleware: [authMiddleware],
  Component: Root,
  children: [{
    path: "admin",
    loader: ({ context }) => {
      const user = context.get(userContext);
      if (!user.isAdmin) throw redirect("/");
      return getAdminData();
    },
    Component: Admin,
  }],
}];
```

## Add ScrollRestoration to Root Layout

Render exactly one ScrollRestoration component in the root layout, before Scripts. Use getKey for custom scroll behavior and preventScrollReset on tab-like links.

```tsx
// Correct: ScrollRestoration in root
export default function Root() {
  return (
    <html>
      <body>
        <Outlet />
        <ScrollRestoration
          getKey={(location) => location.pathname}
        />
        <Scripts />
      </body>
    </html>
  );
}

// Prevent scroll reset for tab-like navigation
<Link to="?tab=settings" preventScrollReset>Settings</Link>
```

## Use useNavigation for Pending Indicators

useNavigation returns the current navigation state (idle, loading, submitting). Use it for global loading indicators and form-specific pending states.

```tsx
// Correct: Global loading indicator + form-specific pending
function Root() {
  const navigation = useNavigation();
  const isNavigating = Boolean(navigation.location);

  return (
    <div>
      {isNavigating && <TopLoadingBar />}
      <div className={navigation.state === "loading" ? "loading" : ""}>
        <Outlet />
      </div>
    </div>
  );
}

function CreateForm() {
  const navigation = useNavigation();
  const isSubmitting = navigation.formAction === "/create";

  return (
    <Form method="post" action="/create">
      <input name="title" />
      <button disabled={isSubmitting}>
        {isSubmitting ? "Creating..." : "Create"}
      </button>
    </Form>
  );
}
```

## Handle Route Errors with isRouteErrorResponse

Throw data() with status codes for expected errors (404, 403). Use isRouteErrorResponse to differentiate HTTP errors from JavaScript errors in the error boundary.

```tsx
// In the loader: throw expected errors
export async function loader({ params }) {
  const record = await db.find(params.id);
  if (!record) {
    throw data("Not Found", { status: 404 });
  }
  return record;
}

// In the error boundary: differentiate error types
export function ErrorBoundary() {
  const error = useRouteError();

  if (isRouteErrorResponse(error)) {
    return <h1>{error.status}: {error.data}</h1>;
  }
  if (error instanceof Error) {
    return <h1>Error: {error.message}</h1>;
  }
  return <h1>Unknown Error</h1>;
}
```

## Add a Catch-All 404 Route

A splat route (`*`) catches all unmatched URLs. Return a 404 status for proper HTTP semantics.

```tsx
createBrowserRouter([
  {
    path: "/",
    Component: Root,
    children: [
      { index: true, Component: Home },
      { path: "about", Component: About },
      {
        path: "*",
        loader: () => { throw data("Page not found", { status: 404 }); },
        Component: NotFound,
      },
    ],
  },
]);
```

## Use NavLink for Active State and viewTransition for Animations

NavLink provides isActive and isPending in its className callback. The viewTransition prop enables CSS View Transitions API animations during navigation.

```tsx
// Correct: NavLink with active state + view transitions
<nav>
  <NavLink
    to="/dashboard"
    viewTransition
    className={({ isActive, isPending }) =>
      isActive ? "active" : isPending ? "pending" : ""
    }
  >
    Dashboard
  </NavLink>
</nav>

// Shared element transitions with useViewTransitionState
function ImageLink({ src, idx }) {
  const href = `/image/${idx}`;
  const isTransitioning = useViewTransitionState(href);

  return (
    <Link to={href} viewTransition>
      <img src={src} style={{
        viewTransitionName: isTransitioning ? "hero-image" : "none",
      }} />
    </Link>
  );
}
```

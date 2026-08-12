# Routing

One Process: use data-aware routing, put reads and writes in route contracts, code-split at route boundaries, and verify navigation states and errors.

## 1. Choose a data-aware router when routes own data

### Use createBrowserRouter for data-aware routing

`createBrowserRouter` enables loaders, actions, pending states, and error handling. It replaces `<BrowserRouter>` for applications that need data loading. Use Framework Mode for file-based routing with automatic code splitting.

Never:
  ```tsx
  <BrowserRouter>
    <Routes>
      <Route path="/" element={<Root />}>
        <Route path="dashboard" element={<Dashboard />} />
      </Route>
    </Routes>
  </BrowserRouter>
  ```

Example:
  ```tsx
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

## 2. Put reads and writes in route handlers

### Use loaders for reads and actions for writes

Loaders fetch data before the route renders. Actions handle form mutations. After an action completes, React Router revalidates all loaders on the page.

Example:
  ```tsx
  createBrowserRouter([{
    path: "/todos",
    loader: async () => db.todos.findAll(),
    action: async ({ request }) => {
      const formData = await request.formData();
      await db.todos.create({ title: formData.get("title") });
      return { ok: true };
    },
    Component: Todos,
  }]);

  function Todos({ loaderData }) {
    return (
      <div>
        <ul>{loaderData.map(todo => <li key={todo.id}>{todo.title}</li>)}</ul>
        <Form method="post">
          <input name="title" />
          <button type="submit">Add</button>
        </Form>
      </div>
    );
  }
  ```

### Use useFetcher for inline mutations

`useFetcher` submits data without navigating or creating history entries. Use it for toggles, inline edits, and background operations.

Never:
  ```tsx
  <Form method="post" action={`/todos/${todo.id}/toggle`}>
    <button type="submit">Toggle</button>
  </Form>
  ```

Example:
  ```tsx
  function TodoItem({ todo }) {
    const fetcher = useFetcher();
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

## 3. Load route code at route boundaries

### Lazy load feature routes

Use the `lazy` property to code-split routes. Keep the shell, root layout, and navigation eagerly loaded. Lazy loads both component and loader before rendering, avoiding nested spinners.

Example:
  ```tsx
  createBrowserRouter([
    {
      path: "/",
      Component: Layout,
      children: [
        { index: true, Component: Home },
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

## 4. Protect routes at the route boundary

### Use middleware for authentication and loader redirects for authorization

Middleware handles cross-cutting authentication. Loader redirects handle route-specific authorization. Throw `redirect()` to stop execution immediately.

Example:
  ```tsx
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

## 5. Preserve navigation state

### Add ScrollRestoration to the root layout

Render exactly one `ScrollRestoration` component in the root layout before `Scripts`. Use `getKey` for custom scroll behavior and `preventScrollReset` on tab-like links.

Example:
  ```tsx
  export default function Root() {
    return (
      <html>
        <body>
          <Outlet />
          <ScrollRestoration getKey={(location) => location.pathname} />
          <Scripts />
        </body>
      </html>
    );
  }

  <Link to="?tab=settings" preventScrollReset>Settings</Link>
  ```

### Use useNavigation for pending indicators

`useNavigation` returns the current navigation state: idle, loading, or submitting. Use it for global loading indicators and form-specific pending states.

Example:
  ```tsx
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

## 6. Handle route errors and missing routes

### Use isRouteErrorResponse for expected HTTP errors

Throw `data()` with status codes for expected errors such as 404 or 403. Use `isRouteErrorResponse` to differentiate HTTP errors from JavaScript errors in the error boundary.

Example:
  ```tsx
  export async function loader({ params }) {
    const record = await db.find(params.id);
    if (!record) {
      throw data("Not Found", { status: 404 });
    }
    return record;
  }

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

### Add a catch-all 404 route

A splat route (`*`) catches unmatched URLs. Return a 404 status for HTTP semantics.

Example:
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

## 7. Expose active and animated navigation states

### Use NavLink for active state and viewTransition for animations

`NavLink` provides `isActive` and `isPending` in its `className` callback. The `viewTransition` prop enables CSS View Transitions API animations during navigation.

Example:
  ```tsx
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

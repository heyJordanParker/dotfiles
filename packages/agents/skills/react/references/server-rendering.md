# Server Rendering

One Process: keep the server-client edge low, pass only serializable data, validate every public server call, and verify client bundle boundaries.

## 1. Keep components on the server by default

### Server Components are the default

In frameworks like Next.js App Router, components are Server Components by default and need no directive. Add `"use client"` only when a component needs interactivity: hooks, event handlers, or browser APIs.

Never:
  ```tsx
  "use client";
  export default function ProductList({ products }) {
    return <ul>{products.map(product => <li key={product.id}>{product.name}</li>)}</ul>
  }
  ```

Example:
  ```tsx
  export default function ProductList({ products }) {
    return <ul>{products.map(product => <li key={product.id}>{product.name}</li>)}</ul>
  }
  ```

### Push the client boundary down

Only the interactive leaf needs `"use client"`. Keep the server-client edge as low as possible to maximize server rendering and minimize the client bundle.

Example:
  ```tsx
  export default function ProductPage({ product }) {
    return (
      <div>
        <h1>{product.name}</h1>
        <p>{product.description}</p>
        <AddToCartButton id={product.id} />
      </div>
    )
  }

  "use client";
  export function AddToCartButton({ id }) {
    return <button onClick={() => addToCart(id)}>Add to Cart</button>
  }
  ```

## 2. Minimize data crossing the boundary

### Pass only serialized fields the client uses

Props crossing the server-client edge must be serializable. Send only the fields the client actually uses.

Never:
  ```tsx
  const user = await db.users.find(userId)
  return <UserCard user={user} />
  ```

Example:
  ```tsx
  return <UserCard name={user.name} avatarUrl={user.avatarUrl} />
  ```

### Transform on the server and render on the client

Do heavy transformations in the Server Component. Do not serialize raw data and transform it on the client.

Example:
  ```tsx
  export default async function Chart() {
    const rawData = await getAnalytics()
    const chartPoints = rawData.map(row => ({ x: row.timestamp, y: row.value }))
    return <ChartClient points={chartPoints} />
  }
  ```

## 3. Mark server code precisely

### use server marks functions, not components

`"use server"` creates Server Actions: async functions executed on the server. It does not make a component a Server Component.

Example:
  ```tsx
  export default function Dashboard() {
    async function deleteItem(id) {
      "use server";
      await db.items.delete(id)
    }
    return <DeleteButton action={deleteItem} />
  }
  ```

Example:
  ```tsx
  "use server";
  export async function createPost(formData) { /* ... */ }
  export async function deletePost(id) { /* ... */ }
  ```

### Use server-only to prevent client bundling

Example:
  ```tsx
  import "server-only"

  export function getUsers() {
    return fetch(API_URL, { headers: { Authorization: process.env.DB_SECRET } })
  }
  ```

## 4. Fetch in parallel

### Use composition for parallel fetching

Do not await sequentially in a parent. Let each async Server Component fetch independently.

Never:
  ```tsx
  export default async function Dashboard() {
    const user = await getUser()
    const posts = await getPosts()
    return <div><UserCard user={user} /><PostList posts={posts} /></div>
  }
  ```

Example:
  ```tsx
  export default function Dashboard() {
    return (
      <div>
        <Suspense fallback={<UserSkeleton />}><UserCard /></Suspense>
        <Suspense fallback={<PostSkeleton />}><PostList /></Suspense>
      </div>
    )
  }

  async function UserCard() {
    const user = await getUser()
    return <div>{user.name}</div>
  }
  ```

### Use React cache for per-request deduplication

When multiple components need the same data in one request, `cache()` deduplicates the fetch.

Example:
  ```tsx
  import { cache } from "react"

  const getUser = cache(async () => {
    return await db.users.findCurrent()
  })

  async function Header() { const user = await getUser(); return <div>{user.name}</div> }
  async function Sidebar() { const user = await getUser(); return <div>{user.role}</div> }
  ```

### Hoist static input and output to module scope

Data that does not change per request can be fetched at module scope and shared across requests.

Example:
  ```tsx
  const config = await fetchConfig()

  export default function App() {
    return <div theme={config.theme}>...</div>
  }
  ```

Use for fonts, logos, config files, and email Templates. Do not use for per-User data, runtime-changing files, or sensitive data.

## 5. Treat Server Actions as public endpoints

### Validate, authenticate, authorize, execute

Server Actions are public HTTP endpoints. Treat all arguments as untrusted. TypeScript types are erased at build time and provide no runtime protection.

Example:
  ```tsx
  "use server";
  import { z } from "zod"

  const DeletePostSchema = z.object({ postId: z.string().uuid() })

  export async function deletePost(rawInput: unknown) {
    const { postId } = DeletePostSchema.parse(rawInput)
    const session = await getSession()
    if (!session) throw new Error("Not authenticated")
    const post = await db.posts.find(postId)
    if (post.authorId !== session.userId) throw new Error("Not authorized")
    await db.posts.delete(postId)
  }
  ```

### Build a data access layer

Centralize data access behind authorized functions. Server Actions call the data access layer, which enforces authentication and authorization.

Example:
  ```tsx
  import "server-only"

  export async function updateUserEmail(userId: string, email: string) {
    const session = await getSession()
    if (!session || session.userId !== userId) throw new Error("Unauthorized")
    return db.users.update(userId, { email: z.string().email().parse(email) })
  }
  ```

### Server Actions are POST-only

React sends Server Action requests as POST only. Frameworks add cross-site request forgery protection through Origin and Host checking. Use Server Actions for mutations, not reads.

## 6. Protect secrets crossing server and client

### Use taint APIs for secret leakage prevention

Example:
  ```tsx
  import { experimental_taintUniqueValue } from "react"

  experimental_taintUniqueValue(
    "Do not pass the API token to the client.",
    process,
    process.env.API_SECRET
  )
  ```

`taintObjectReference` prevents entire objects from reaching Client Components. Derived values such as `.toUpperCase()`, spread, and destructure bypass tainting, so tainting is not a sole defense.

### Bound Server Action arguments are not encrypted protection

Values passed through `.bind()` on Server Actions are visible in the client's network tab. Authenticate through session, not bound arguments.

### Closures are encrypted but not authorized

When a Server Action closes over Server Component variables, Next.js 14 and later encrypts those values in transit. Encryption is not authorization; verify the session on every call.

## 7. Choose server-side or client-side fetching by ownership

### Fetch initial and protected data on the server

Server Components own initial page data, static content, and authentication-gated data with direct database access.

### Fetch interactive data on the client

Client Components own interactive search, filtering, polling, optimistic mutations, and data depending on User interaction.

### Use a hybrid only when both sides own behavior

Fetch server-side and pass `initialData` to TanStack Query for client-side mutations and revalidation.

# Server Components and Actions

## Server Components Are the Default

In frameworks like Next.js App Router, components are Server Components by default. They run on the server with no directive. Only add `"use client"` when the component needs interactivity (hooks, event handlers, browser APIs).

```tsx
// Incorrect: Unnecessary client directive on static component
"use client";
export default function ProductList({ products }) {
  return <ul>{products.map(p => <li key={p.id}>{p.name}</li>)}</ul>
}

// Correct: Server Component by default
export default function ProductList({ products }) {
  return <ul>{products.map(p => <li key={p.id}>{p.name}</li>)}</ul>
}
```

## Push the Client Boundary Down

Only the interactive leaf needs `"use client"`. Keep the boundary as low as possible to maximize server rendering and minimize client bundle.

```tsx
// Correct: Only the button is a client component
export default function ProductPage({ product }) {
  return (
    <div>
      <h1>{product.name}</h1>
      <p>{product.description}</p>
      <AddToCartButton id={product.id} />
    </div>
  )
}

// AddToCartButton.tsx
"use client";
export function AddToCartButton({ id }) {
  return <button onClick={() => addToCart(id)}>Add to Cart</button>
}
```

## Minimize Serialized Props

Props crossing the server-client boundary must be serializable. Only pass the fields the client actually uses.

```tsx
// Incorrect: Entire database object sent to client
const user = await db.users.find(userId)
return <UserCard user={user} />

// Correct: Only needed fields
return <UserCard name={user.name} avatarUrl={user.avatarUrl} />
```

## "use server" Marks Functions, Not Components

`"use server"` creates Server Actions (async functions executed on the server). It does NOT make a component a Server Component.

```tsx
// At function level — inline in a Server Component
export default function Dashboard() {
  async function deleteItem(id) {
    "use server";
    await db.items.delete(id)
  }
  return <DeleteButton action={deleteItem} />
}

// At file level — module of Server Actions
// actions.ts
"use server";
export async function createPost(formData) { /* ... */ }
export async function deletePost(id) { /* ... */ }
```

## Parallel Fetching via Composition

Don't await sequentially in a parent. Let each async Server Component fetch independently.

```tsx
// Incorrect: Sequential waterfall
export default async function Dashboard() {
  const user = await getUser()
  const posts = await getPosts()
  return <div><UserCard user={user} /><PostList posts={posts} /></div>
}

// Correct: Parallel — each component fetches independently
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

## React.cache() for Per-Request Deduplication

When multiple components need the same data in one request, `cache()` deduplicates the fetch.

```tsx
import { cache } from "react"

const getUser = cache(async () => {
  return await db.users.findCurrent()
})

// Both components call getUser() — only one DB query executes
async function Header() { const user = await getUser(); return <div>{user.name}</div> }
async function Sidebar() { const user = await getUser(); return <div>{user.role}</div> }
```

## Transform on Server, Render on Client

Do heavy transformations in the Server Component. Don't serialize raw data and transform on the client.

```tsx
// Correct: Transform on server, send only what client needs
export default async function Chart() {
  const rawData = await getAnalytics()
  const chartPoints = rawData.map(d => ({ x: d.timestamp, y: d.value }))
  return <ChartClient points={chartPoints} />
}
```

## Hoist Static I/O to Module Scope

Data that doesn't change per-request (config, feature flags) can be fetched at module scope. Runs once at import time, shared across all requests.

```tsx
const config = await fetchConfig() // Runs once at import time

export default function App() {
  return <div theme={config.theme}>...</div>
}
```

Use for: fonts, logos, config files, email templates. Don't use for: per-user data, runtime-changing files, sensitive data.

## Server Actions Are Public HTTP Endpoints

Treat all arguments as untrusted. Always: validate → authenticate → authorize → execute.

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

TypeScript types are erased at build time — they provide zero runtime protection.

## Use server-only to Prevent Client Bundling

```tsx
import "server-only"  // Build error if this module is ever bundled for client

export function getUsers() {
  return fetch(API_URL, { headers: { Authorization: process.env.DB_SECRET } })
}
```

## Taint APIs for Secret Leakage Prevention

```tsx
import { experimental_taintUniqueValue } from "react"

experimental_taintUniqueValue(
  "Do not pass the API token to the client.",
  process,
  process.env.API_SECRET
)
```

`taintObjectReference` prevents entire objects from reaching Client Components. Derived values (`.toUpperCase()`, spread, destructure) bypass tainting — not a sole defense.

## .bind() Arguments Are Not Encrypted

Values passed via `.bind()` on Server Actions are visible in the client's network tab. Authenticate via session, not bound arguments.

## Build a Data Access Layer

Centralize data access behind authorized functions. Server Actions call the DAL, which enforces auth.

```tsx
// dal.ts
import "server-only"

export async function updateUserEmail(userId: string, email: string) {
  const session = await getSession()
  if (!session || session.userId !== userId) throw new Error("Unauthorized")
  return db.users.update(userId, { email: z.string().email().parse(email) })
}
```

## Server Actions Are POST-Only

React sends Server Action requests as POST only. Frameworks add CSRF protection (Origin/Host checking) automatically. Use Server Actions for mutations, not data reading.

## When to Fetch Server-Side vs Client-Side

- **Server Components:** initial page data, static content, auth-gated data with direct DB access
- **Client Components:** interactive search/filter, polling, optimistic mutations, data depending on user interaction
- **Hybrid:** fetch server-side, pass as `initialData` to TanStack Query for client-side mutations and revalidation

## Closures Are Encrypted but Not Authorized

When a Server Action closes over Server Component variables, Next.js 14+ encrypts those values in transit. Encryption is not authorization — always verify the session on every call.

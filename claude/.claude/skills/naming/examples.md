# Naming Examples

## Abbreviations

- `usr` → `user` — don't shorten common words
- `btn` → `button` — spell it out
- `msg` → `message` — readability matters
- `cfg` → `config` — `config` is acceptable (universally understood)
- `info` → `info` — acceptable (universally understood)
- `maxRetries` → `maxRetries` — `max`/`min` are acceptable

## Context Provides Meaning

- `user.isUserValid()` → `user.isValid()` — container already says "user"
- `user.getUserName()` → `user.name` — redundant prefix
- `date-utils.ts` → `utils/dates.ts` — folder provides context
- `UserManager.createNewUser()` → `UserManager.create()` — "New" and "User" are implied

## No Redundant Suffixes

- `userList` → `users` — type tells you it's a list
- `userArray` → `users` — type tells you it's an array
- `orderData` → `order` — "Data" adds nothing
- `configObject` → `config` — "Object" adds nothing

## Hide Implementation

- `fetchAndCacheUser` → `getUser` — caching is implementation
- `queryDatabaseForOrders` → `getOrders` — database is implementation
- `parseJsonResponse` → `parseResponse` — JSON is implementation

## Booleans

- `loggedIn` → `isLoggedIn` — prefix clarifies it's boolean
- `permissions` → `hasPermission` — prefix clarifies it's boolean
- `editable` → `canEdit` — prefix clarifies capability
- `visible` → `isVisible` — prefix clarifies state

## Event Handlers

- **Internal handler** — `handle` + Event (e.g., `handleSubmit`, `handleClick`)
- **Callback prop** — `on` + Event (e.g., `onSubmit`, `onClick`, `onOpenChange`)

## Hooks

- `productData()` → `useProducts()` — `use` prefix is React convention
- `useGetProducts()` → `useProducts()` — "Get" is implied
- `useProductsHook()` → `useProducts()` — "Hook" is redundant

## Collections

- `userList` → `users` — simple plural
- `orderItems` → `items` (in Order context) — context provides meaning
- `allUsers` → `users` — "all" rarely adds meaning

## Transformers

- `convertUserToJson(user)` → `user.toJson()` — method on the source
- `userToResponse(user)` → `user.toResponse()` — method on the source
- `formatOrderData(order)` → `order.format()` — method on the source

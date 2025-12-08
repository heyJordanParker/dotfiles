# Naming Examples

## Abbreviations

| Bad | Good | Why |
|-----|------|-----|
| `usr` | `user` | Don't shorten common words |
| `btn` | `button` | Spell it out |
| `msg` | `message` | Readability matters |
| `cfg` | `config` | `config` is acceptable (universally understood) |
| `info` | `info` | Acceptable (universally understood) |
| `maxRetries` | `maxRetries` | `max`/`min` are acceptable |

## Context Provides Meaning

| Bad | Good | Why |
|-----|------|-----|
| `user.isUserValid()` | `user.isValid()` | Container already says "user" |
| `user.getUserName()` | `user.name` | Redundant prefix |
| `date-utils.ts` | `utils/dates.ts` | Folder provides context |
| `UserManager.createNewUser()` | `UserManager.create()` | "New" and "User" are implied |

## No Redundant Suffixes

| Bad | Good | Why |
|-----|------|-----|
| `userList` | `users` | Type tells you it's a list |
| `userArray` | `users` | Type tells you it's an array |
| `orderData` | `order` | "Data" adds nothing |
| `configObject` | `config` | "Object" adds nothing |

## Hide Implementation

| Bad | Good | Why |
|-----|------|-----|
| `fetchAndCacheUser` | `getUser` | Caching is implementation |
| `queryDatabaseForOrders` | `getOrders` | Database is implementation |
| `parseJsonResponse` | `parseResponse` | JSON is implementation |

## Booleans

| Bad | Good | Why |
|-----|------|-----|
| `loggedIn` | `isLoggedIn` | Prefix clarifies it's boolean |
| `permissions` | `hasPermission` | Prefix clarifies it's boolean |
| `editable` | `canEdit` | Prefix clarifies capability |
| `visible` | `isVisible` | Prefix clarifies state |

## Event Handlers

| Context | Pattern | Example |
|---------|---------|---------|
| Internal handler | `handle` + Event | `handleSubmit`, `handleClick` |
| Callback prop | `on` + Event | `onSubmit`, `onClick`, `onOpenChange` |

## Hooks

| Bad | Good | Why |
|-----|------|-----|
| `productData()` | `useProducts()` | `use` prefix is React convention |
| `useGetProducts()` | `useProducts()` | "Get" is implied |
| `useProductsHook()` | `useProducts()` | "Hook" is redundant |

## Collections

| Bad | Good | Why |
|-----|------|-----|
| `userList` | `users` | Simple plural |
| `orderItems` | `items` (in Order context) | Context provides meaning |
| `allUsers` | `users` | "all" rarely adds meaning |

## Transformers

| Bad | Good | Why |
|-----|------|-----|
| `convertUserToJson(user)` | `user.toJson()` | Method on the source |
| `userToResponse(user)` | `user.toResponse()` | Method on the source |
| `formatOrderData(order)` | `order.format()` | Method on the source |

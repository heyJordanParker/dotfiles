# TypeScript Patterns

Anti-slop patterns specific to TypeScript codebases.

## Type Escapes

**Slop:** Bypassing the type system instead of fixing it.

```typescript
// Slop - any
const data: any = fetchData();
data.whatever.you.want;  // No safety

// Slop - double cast
const user = data as unknown as User;

// Slop - non-null assertion
const name = user!.name;  // Crashes if null

// Clean - proper typing
interface ApiResponse {
  user: User | null;
}
const data: ApiResponse = await fetchData();
const name = data.user?.name ?? 'Anonymous';
```

| Escape | Fix |
|--------|-----|
| `any` | Define interface/type |
| `as unknown as X` | Fix the actual type mismatch |
| `x!` | Optional chaining `x?.` or null check |
| `// @ts-ignore` | Fix the type error |
| `as X` | Narrow with type guards |

## Null Handling

**Slop:** Pretending nulls don't exist.

```typescript
// Slop - assertion
const user = getUser()!;
console.log(user.name);

// Slop - silent undefined
const users = data.users;  // Could be undefined
users.map(...);  // Runtime crash

// Clean - explicit handling
const user = getUser();
if (!user) {
  throw new Error('User not found');
}
console.log(user.name);

// Clean - optional chaining + default
const count = data.users?.length ?? 0;
```

## Reinventing Built-ins

**Slop:** Custom utils when standard methods exist.

```typescript
// Slop - manual loop
const filtered = [];
for (const item of items) {
  if (item.active) {
    filtered.push(item);
  }
}

// Clean
const filtered = items.filter(item => item.active);

// Slop - custom find
let found = null;
for (const item of items) {
  if (item.id === targetId) {
    found = item;
    break;
  }
}

// Clean
const found = items.find(item => item.id === targetId);
```

| Instead of | Use |
|------------|-----|
| Loop + push | `.filter()`, `.map()` |
| Loop + break | `.find()`, `.findIndex()` |
| Loop + counter | `.reduce()`, `.length` |
| Loop + boolean | `.some()`, `.every()` |
| Manual includes check | `.includes()`, `Set.has()` |
| String concatenation loop | `.join()` |
| `Object.keys().forEach()` | `Object.entries()` |

## Object/Array Patterns

**Slop:** Verbose mutations.

```typescript
// Slop - manual object merge
const config = {
  host: defaults.host,
  port: defaults.port,
  timeout: options.timeout,
};

// Clean - spread
const config = { ...defaults, ...options };

// Slop - manual array concat
const all = [];
all.push(...first);
all.push(...second);

// Clean
const all = [...first, ...second];

// Slop - manual clone + modify
const updated = JSON.parse(JSON.stringify(obj));
updated.value = newValue;

// Clean
const updated = { ...obj, value: newValue };
```

## Async Patterns

**Slop:** Mixing paradigms or ignoring errors.

```typescript
// Slop - .then() when async/await available
function getData() {
  return fetch(url)
    .then(res => res.json())
    .then(data => process(data));
}

// Clean
async function getData() {
  const res = await fetch(url);
  const data = await res.json();
  return process(data);
}

// Slop - fire and forget
async function save() {
  saveToDb(data);  // Missing await!
}

// Slop - catch swallowing
try {
  await riskyOperation();
} catch (e) {
  console.log(e);  // Swallowed
}

// Clean
try {
  await riskyOperation();
} catch (e) {
  logger.error('Operation failed', e);
  throw e;  // Or handle meaningfully
}
```

## Imports

**Slop:** Inconsistent or bloated imports.

```typescript
// Slop - import all
import * as _ from 'lodash';
_.get(obj, 'path');

// Clean - named import
import { get } from 'lodash';
get(obj, 'path');

// Even better - often built-in works
obj.path ?? defaultValue;

// Slop - mixed styles
import React from 'react';
import { useState } from 'react';

// Clean - consistent
import { useState, useEffect } from 'react';
```

## Type Definitions

**Slop:** Overly loose or redundant types.

```typescript
// Slop - object as type
function process(data: object) { ... }

// Slop - inline repeated type
function getUser(): { id: string; name: string } { ... }
function updateUser(user: { id: string; name: string }) { ... }

// Clean - named type
interface User {
  id: string;
  name: string;
}
function getUser(): User { ... }
function updateUser(user: User) { ... }

// Slop - any in generic
function wrap<T = any>(value: T) { ... }

// Clean - constrained or inferred
function wrap<T>(value: T) { ... }
```

## Quick Reference

| Slop | Fix |
|------|-----|
| `: any` | Define interface |
| `as unknown as X` | Fix type mismatch |
| `x!.prop` | `x?.prop` or null check |
| `// @ts-ignore` | Fix the error |
| Manual loop | Array methods |
| `.then()` chains | `async/await` |
| `catch(e) { log(e) }` | Handle or rethrow |
| `import *` | Named imports |
| Inline type literals | Named interface |
| `object` type | Specific interface |

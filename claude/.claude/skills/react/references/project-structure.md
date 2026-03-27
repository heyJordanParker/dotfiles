# Project Structure Conventions

## Organize by Feature, Not by File Type

Group code by business domain, not technical layer. Feature-based structure scales to large teams, enables independent ownership, and makes features easy to delete.

Incorrect — layer-based:
```
src/components/UserProfile.tsx, PostList.tsx, PaymentForm.tsx
src/hooks/useUser.ts, usePosts.ts, usePayment.ts
src/types/user.ts, post.ts, payment.ts
```

Correct — feature-based:
```
src/
  features/user/ (components/, hooks/, api/, types/)
  features/post/ (components/, hooks/, api/, types/)
  components/    # shared UI primitives only
  hooks/         # shared hooks only
  utils/         # shared pure functions only
```

Start layer-based for small projects. Migrate to feature-based at 3+ features or multiple developers.

## Colocate Related Files

Keep a component's test, styles, and types next to it. Deleting a component should delete everything related.

Incorrect:
```
src/components/UserProfile.tsx
src/tests/UserProfile.test.tsx
src/styles/UserProfile.css
```

Correct:
```
src/features/user/components/UserProfile/
  UserProfile.tsx
  UserProfile.test.tsx
  UserProfile.module.css
```

Feature-specific hooks stay in the feature. Only hooks used by 2+ features go in the shared `hooks/` directory.

## Do Not Use Barrel Files in Application Code

Barrel files (`index.ts` re-exporting submodules) force JavaScript to load every module synchronously, even when only one export is used.

- **Vercel:** 200-800ms per import; dev startup 72% slower with barrels
- **Atlassian:** 75% build time reduction after removing barrels across 90,000+ files
- **TkDodo:** 11,000 modules at startup dropped to 3,500 after removal (-68%)

Tree-shaking doesn't help — test runners load everything, and bundlers can't optimize externals.

Incorrect:
```typescript
// features/user/index.ts
export { UserProfile } from './components/UserProfile';
export { useUser } from './hooks/useUser';
```

Correct — import directly:
```typescript
import { UserProfile } from '@/features/user/components/UserProfile';
import { useUser } from '@/features/user/hooks/useUser';
```

Barrel files are acceptable only for published library APIs.

## Use Named Exports, Not Default Exports

Named exports are grep-able, refactoring-safe, and prevent silent renames at import sites.

Incorrect:
```typescript
export default function UserProfile() { ... }
import Banana from './UserProfile'; // no error, breaks grep
```

Correct:
```typescript
export function UserProfile() { ... }
import { UserProfile } from './UserProfile';
```

For `React.lazy` (requires default), wrap the import:
```typescript
const UserProfile = lazy(() =>
  import('./UserProfile').then(mod => ({ default: mod.UserProfile }))
);
```

## Follow File Naming Conventions

- Components: PascalCase (`UserProfile.tsx`)
- Hooks: camelCase with `use` prefix (`useAuth.ts`)
- Utilities: camelCase (`formatDate.ts`)
- Tests: source name + `.test` (`UserProfile.test.tsx`)
- Styles: source name + `.module` (`UserProfile.module.css`)
- Route segments: kebab-case (`user-profile/page.tsx`)

Pick one convention and enforce it project-wide. Consistency matters more than which you choose.

## Use Absolute Imports via @/ Prefix

Relative imports with `../../../` are unreadable and break when files move.

```json
{ "compilerOptions": { "baseUrl": ".", "paths": { "@/*": ["./src/*"] } } }
```

```typescript
import { Button } from '@/components/Button';
import { useAuth } from '@/features/auth/hooks/useAuth';
```

Use relative imports only within the same directory: `./UserAvatar`.

## One Component per File, Split at ~150 Lines

Each file exports one component. Small sub-components (<30 lines, parent-only) can stay as private helpers.

Split when a component exceeds ~150 lines, has its own state, or is reused elsewhere:
```
UserProfile/
  UserProfile.tsx          # exported
  UserProfileAvatar.tsx    # internal, not exported
  UserProfile.test.tsx
```

## Enforce Unidirectional Dependencies

Code flows: `shared -> features -> app`. Features never import other features. Enforce with ESLint:

```json
{
  "rules": {
    "import/no-restricted-paths": ["error", {
      "zones": [
        { "target": "./src/features", "from": "./src/app" },
        { "target": "./src/features/user", "from": "./src/features/post" }
      ]
    }]
  }
}
```

## Follow a Consistent Component File Order

```typescript
// 1. Imports (React, third-party, project, local)
// 2. Types
// 3. Constants
// 4. Component (exported)
export function UserProfile({ user }: Props) {
  // a. Hooks → b. Derived state → c. Event handlers → d. Return JSX
}
// 5. Private sub-components
```

## What NOT to Do

**Over-nesting** — max 2-3 levels within any feature. If an import path has 4+ segments after `src/`, it's too deep.

**Premature directories** — create a directory when you have 2+ files. A folder with one file is noise.

**Generic dumping grounds** — never create `helpers/`, `common/`, `misc/`. They become junk drawers.

**Mirroring backend** — don't use `models/`, `controllers/` on the frontend. Organize by UI concerns.

**File structure as access control** — use module exports, not `public/`/`private/` directories.

**Anticipating complexity** — match current complexity, not future. Restructure when pain appears.

# Project Architecture

One Process: organize by User capability, colocate owned files, keep imports direct, and enforce one-way dependencies.

## 1. Organize by feature until the project proves otherwise

### Group code by business domain, not file type

Feature-based organization supports ownership and deletion. Start layer-based only for small projects; move to feature-based at three or more features or multiple developers.

Never:
  ```text
  src/components/UserProfile.tsx, PostList.tsx, PaymentForm.tsx
  src/hooks/useUser.ts, usePosts.ts, usePayment.ts
  src/types/user.ts, post.ts, payment.ts
  ```

Example:
  ```text
  src/
    features/user/ (components/, hooks/, api/, types/)
    features/post/ (components/, hooks/, api/, types/)
    components/    # shared User Interface primitives only
    hooks/         # shared hooks only
    utils/         # shared pure functions only
  ```

## 2. Colocate files that share ownership

### Keep tests, styles, and types beside the component

Deleting a component should delete everything related to it. Feature-specific hooks stay in the feature. Only hooks used by two or more features go in shared `hooks/`.

Never:
  ```text
  src/components/UserProfile.tsx
  src/tests/UserProfile.test.tsx
  src/styles/UserProfile.css
  ```

Example:
  ```text
  src/features/user/components/UserProfile/
    UserProfile.tsx
    UserProfile.test.tsx
    UserProfile.module.css
  ```

## 3. Keep imports direct

### Do not use barrel files in application code

Barrel files (`index.ts` re-exporting submodules) force JavaScript to load every module synchronously, even when only one export is used. Tree-shaking does not help because test runners load everything and bundlers cannot optimize externals.

Measured: Vercel found 200 to 800 milliseconds per import and 72 percent slower development startup with barrels. Atlassian reduced build time 75 percent after removing barrels across more than 90,000 files. TkDodo saw 11,000 startup modules drop to 3,500, a 68 percent reduction.

Never:
  ```typescript
  export { UserProfile } from './components/UserProfile';
  export { useUser } from './hooks/useUser';
  ```

Example:
  ```typescript
  import { UserProfile } from '@/features/user/components/UserProfile';
  import { useUser } from '@/features/user/hooks/useUser';
  ```

Barrel files are acceptable only for published library APIs.

### Use named exports, not default exports

Named exports are grep-able, refactoring-safe, and prevent silent renames at import sites.

Never:
  ```typescript
  export default function UserProfile() { ... }
  import Banana from './UserProfile';
  ```

Example:
  ```typescript
  export function UserProfile() { ... }
  import { UserProfile } from './UserProfile';
  ```

Example for `React.lazy` boundaries that require default:
  ```typescript
  const UserProfile = lazy(() =>
    import('./UserProfile').then(module => ({ default: module.UserProfile }))
  );
  ```

### Use absolute imports via @ prefix

Relative imports with `../../../` are unreadable and break when files move.

Example:
  ```json
  { "compilerOptions": { "baseUrl": ".", "paths": { "@/*": ["./src/*"] } } }
  ```

Example:
  ```typescript
  import { Button } from '@/components/Button';
  import { useAuth } from '@/features/auth/hooks/useAuth';
  ```

Use relative imports only within the same directory, such as `./UserAvatar`.

## 4. Name files consistently

### Follow one project-wide naming convention

Components use PascalCase (`UserProfile.tsx`). Hooks use camelCase with a `use` prefix (`useAuth.ts`). Utilities use camelCase (`formatDate.ts`). Tests use source name plus `.test` (`UserProfile.test.tsx`). Styles use source name plus `.module` (`UserProfile.module.css`). Route segments use kebab-case (`user-profile/page.tsx`).

Pick one convention and enforce it project-wide. Consistency matters more than which convention is chosen.

## 5. Split files by ownership

### Use one component per file

Each file exports one component. Small subcomponents under about 30 lines and used only by the parent can stay private.

### Split around 150 lines or when ownership changes

Split when a component exceeds about 150 lines, has its own state, or is reused elsewhere.

Example:
  ```text
  UserProfile/
    UserProfile.tsx
    UserProfileAvatar.tsx
    UserProfile.test.tsx
  ```

## 6. Enforce one-way dependencies

### Dependencies move from shared to features to app

Features never import other features. Enforce this with lint rules.

Example:
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

## 7. Keep component files predictable

### Follow consistent component file order

Template:
  ```typescript
  // 1. Imports: React, third-party, project, local
  // 2. Types
  // 3. Constants
  // 4. Component: exported
  export function UserProfile({ user }: Props) {
    // a. Hooks → b. Derived state → c. Event handlers → d. Return JSX
  }
  // 5. Private subcomponents
  ```

## 8. Cut shapes that hide ownership

### Avoid over-nesting

Keep at most two or three levels within any feature. An import path with four or more segments after `src/` is too deep.

### Avoid premature directories

Create a directory when there are two or more files. A folder with one file is noise.

### Avoid generic dumping grounds

Never create `helpers/`, `common/`, or `misc/`. They become junk drawers.

### Do not mirror backend layers

Do not use `models/` or `controllers/` on the frontend. Organize by User Interface concerns.

### Do not use file organization as access control

Use module exports, not `public/` or `private/` directories.

### Do not anticipate complexity

Match current complexity. Restructure when pain appears.

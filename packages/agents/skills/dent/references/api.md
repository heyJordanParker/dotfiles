# Dent API reference

Dent is schema-driven. The catalog is the contract for entity names, fields, relations, actions, access, and request parameters.

## Catalogs

- Tenant catalog: `GET /api/v1/schema/`

Fetch the catalog before writes. Confirm the entity's fields and action parameters there; for example, Funnel Step `status` is a catalog field and is updated through the generic step update route.

## Tenant route grammar

- `GET /api/v1/{entities}` lists records.
- `GET /api/v1/{entities}/{id}` reads one record.
- `POST /api/v1/{entities}` creates one record.
- `POST /api/v1/{entities}/{id}` updates one record.
- `DELETE /api/v1/{entities}/{id}` deletes one record when allowed.
- `GET|POST|DELETE /api/v1/{entities}/{action}` invokes a collection action.
- `GET|POST|DELETE /api/v1/{entities}/{id}/{action}` invokes a member action.
- Nested routes keep the parent in the path, for example `/api/v1/funnels/{funnelId}/steps`, `/api/v1/courses/{courseId}/sections`, and `/api/v1/sections/{sectionId}/lessons`.

## Public Funnel requests

- Publish public Funnel Steps before opening `viewUrl`, submitting forms, changing the Cart, or submitting Checkout.
- `replace-design` returns Design data, not Step metadata. Read the Step after publishing to confirm `status`, `viewUrl`, and derived flags such as `hasOptinAction` and `hasBuilder`.
- Public form submit path: `GET` the Step `viewUrl` with cookies, then `POST /api/v1/forms/submit` with `{ "owner": {"type": "step", "id": stepId}, "form": "<authored form id>", "fields": {...} }` using those cookies.
- Find a captured Contact with `GET /api/v1/contacts?search=<email>`.
- Every Site render sets the `_vid` visitor cookie server-side, so a cookie-jar `curl` is a tracked Visitor — but only if its `User-Agent` is not bot-flagged (`curl`'s default `curl/*` agent is rejected by the crawler filter; send a real browser `User-Agent`). A tracked Visitor is anonymous until linked to a Contact, which happens on an opt-in or Checkout submit; `{{ contact.* }}` renders resolve only after that submit on the same cookie jar.

## Designer writes

- Design reads return `{owner, design, revision}`. Send `{revision, design}` to `replace-design`; stale revisions return `409` and must be re-read before a deliberate new write.
- Form submit `form` is the form's authored id; the Designer element key `404`s. Preserve the rendered owner, optional holder, and `link_id` hidden values.
- Checkout submits through its Cart: `POST /api/v1/checkout/{cartId}/submit`, using the cookie jar established by the public Checkout pageview.
- Media creation is `POST /api/v1/media/upload-url` → direct `PUT` → `POST /api/v1/media/process`. Direct `POST /api/v1/media` returns guidance instead of creating a record.
- `raw-html` rejects script tags and event-handler attributes at the write boundary (not by silent stripping). URL-bearing attributes allow only `http`, `https`, `mailto`, `tel`, relative, and anchor URLs; other schemes return `422`. JavaScript belongs in a `script` element; `position: "head"` scripts render in the head, `position: "body"` scripts render at the end of the body after page markup.
- `{{ ... }}` tags interpolate only in first-class fields (text, classes, style, attributes); `raw-html` markup and script `code` are emitted verbatim, never interpolated. In a field, unknown `{{ var }}` renders empty and unbound `@root` stays verbatim. Publish validation scans `{{ }}` everywhere (raw-html and script code included), so write `\{{` for a literal `{{` — it passes validation in any carrier and renders as `{{` in a field.
- Theme writes use the public `theme` block keys returned by `GET /api/v1/settings/theme`; unknown theme keys return `422`.

## Analytics dimensions

`GET /api/v1/dent/breakdown` accepts `dimension`: `source`, `referrer`, `campaign`, `country`, `region`, `city`, `device`, `browser`, `entry_pages`.

`dent api` fetches the tenant catalog once per invocation and resolves the HTTP method from the catalog action mode: `read` → `GET`, `write`/`remote` → `POST`, and `destroy` → `DELETE`. `--method` is the explicit override. Bare collection and nested-collection forms list by default; pass `--data` or an explicit method to create.

## Auth

Every direct HTTP request uses bearer auth:

```http
Authorization: Bearer <Dent bearer credential>
Accept: application/json
```

For CLI users, `dent login` starts the OAuth browser flow by default, then verifies the returned credential against the active Dent Site before saving it. Token prompt and stdin login are recovery paths only. Never pass tokens as command-line arguments.

`dent logout` removes the active target's stored credential.

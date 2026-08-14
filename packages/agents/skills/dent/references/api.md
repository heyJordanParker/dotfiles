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
- Publishing a Step or Page, or `replace-design` on one already published, validates the Design and returns `422` with element-keyed messages if publish validation fails (expression syntax, form completeness, script/branch references). Nothing persists on failure. Draft saves keep warnings and always persist.
- `replace-design` returns Design data, not Step metadata. Read the Step after publishing to confirm `status`, `viewUrl`, and derived flags such as `hasOptinAction` and `hasBuilder`.
- Public form submit path: `GET` the Step `viewUrl` with cookies, then `POST /api/v1/forms/submit` with `{ "owner": {"type": "step", "id": stepId}, "form": "form-key", "fields": {...} }` using those cookies.
- Find a captured Contact with `GET /api/v1/contacts?search=<email>`.

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
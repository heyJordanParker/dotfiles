# Dent API reference

Dent is schema-driven. The catalog is the contract for entity names, fields, relations, actions, access, and request parameters.

## Catalogs

- Tenant catalog: `GET /api/v1/schema/`
- Platform catalog: `GET /platform/api/v1/schema/` on the Platform host

## Tenant route grammar

- `GET /api/v1/{entities}` lists records.
- `GET /api/v1/{entities}/{id}` reads one record.
- `POST /api/v1/{entities}` creates one record.
- `POST /api/v1/{entities}/{id}` updates one record.
- `DELETE /api/v1/{entities}/{id}` deletes one record when allowed.
- `GET|POST|DELETE /api/v1/{entities}/{action}` invokes a collection action.
- `GET|POST|DELETE /api/v1/{entities}/{id}/{action}` invokes a member action.
- Nested routes keep the parent in the path, for example `/api/v1/funnels/{funnelId}/steps`, `/api/v1/courses/{courseId}/sections`, and `/api/v1/sections/{sectionId}/lessons`.

## Auth

Every request uses bearer auth:

```http
Authorization: Bearer <personal access token>
Accept: application/json
```

For CSV exports, set `Accept: text/csv`. Never pass tokens as command-line arguments or write them to disk.

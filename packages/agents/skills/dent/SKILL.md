---
name: dent
version: 0.1.0
description: Operate Dent via first-party API. Trigger: Dent API, Funnels, Offers, Pages, Articles, Courses, Spaces, Analytics. Do not trigger for WordPress-only work.
---

# Dent

Dent spins up marketing Sites that sell and deliver digital Products. Operate it through the first-party schema-driven API. Vocabulary is strict: Funnel, Funnel Step, Offer, Product, Order, Cart, Checkout, Journey, Experiment, Conversion, Link, Session, Visitor Event, Attribution, Designer, Design, Designable, Site, Page, Article, Space, Course.

## Process

1. Establish the session before touching data.

   - Run the local checks:

     ```bash
     dent check
     dent whoami
     ```

   - If `dent check` says the skill is stale, tell the user and prefer `dent update` before a write.
   - If `dent whoami` fails, ask for the Dent Site URL and have the user provide a personal access token through a safe channel. Never put a token in argv.

     ```bash
     dent login
     echo "$DENT_PERSONAL_ACCESS_TOKEN" | dent login --site-url https://example.com --stdin
     ```

   - Power users may set both variables; they override stored config:

     ```bash
     export DENT_SITE_URL=https://example.com
     export DENT_API_KEY=...
     ```

2. Fetch the schema catalog first; never guess entity names, fields, actions, or routes.

   - Tenant catalog: `GET /api/v1/schema/`.
   - Platform catalog: `GET /platform/api/v1/schema/` on the Platform host.
   - Local dev tenant: `https://testing.dent.lndo.site`. The bare `https://dent.lndo.site` is Platform and 404s tenant routes.
   - Read the target entity's `fields`, `actions`, `parameters`, `mode`, `target`, and `access` before writing.
   - Action bodies are top-level fields. Do not wrap them in `{attributes: ...}` unless the catalog parameter is literally named `attributes` and the action expects that wrapper from a non-generic endpoint. For Dent generic create/update/actions, send the fields directly.

   ```bash
   dent schema
   dent schema funnels
   dent api funnels
   ```

3. Use the generic route grammar the catalog confirms.

   - `GET /api/v1/{entities}` lists records.
   - `GET /api/v1/{entities}/{id}` reads one record.
   - `POST /api/v1/{entities}` creates a record.
   - `POST /api/v1/{entities}/{id}` updates a record.
   - `DELETE /api/v1/{entities}/{id}` deletes when the catalog and the user intent allow it.
   - `GET|POST|DELETE /api/v1/{entities}/{action}` invokes a collection action.
   - `GET|POST|DELETE /api/v1/{entities}/{id}/{action}` invokes a member action.
   - Nested creates/reads use the parent route the catalog exposes, for example:
     - `POST /api/v1/funnels/{funnelId}/steps`
     - `POST /api/v1/funnels/{funnelId}/steps/{stepId}`
     - `POST /api/v1/courses/{courseId}/sections`
     - `GET /api/v1/courses/{courseId}/sections`
     - `GET /api/v1/courses/{courseId}/sections/{sectionId}`
     - `POST /api/v1/sections/{sectionId}/lessons`
     - `GET /api/v1/sections/{sectionId}/lessons`
     - `GET /api/v1/sections/{sectionId}/lessons/{lessonId}`

4. Build an opt-in Funnel with a thank-you Funnel Step.

   - Create the Funnel: `POST /api/v1/funnels` with `{ "name": "Lead magnet opt-in" }`.
   - Use `homeStepId` from the response as the opt-in Funnel Step.
   - Create the thank-you Funnel Step: `POST /api/v1/funnels/{funnelId}/steps` with `{ "title": "Thank You" }`.
   - Author the opt-in Design through the Funnel Step, not WordPress:

     ```json
     {
       "design": {
         "version": 1,
         "elements": [{
           "key": "optin-section",
           "type": "section",
           "children": [{
             "key": "optin-form",
             "type": "form",
             "behaviors": [{"behavior": "optin", "event": "submit", "config": {"email": "{{ fields.email }}", "name": "{{ fields.name }}", "nameMode": "full"}}],
             "children": [
               {"key": "optin-email", "type": "text-input", "config": {"name": "email", "kind": "email", "label": "Email", "required": true}, "children": []},
               {"key": "optin-submit", "type": "form-submit", "config": {"label": "Subscribe"}, "children": []}
             ]
           }]
         }]
       }
     }
     ```

     Send it to `POST /api/v1/funnels/{funnelId}/steps/{stepId}/replace-design`.
   - Read the Funnel Step with `GET /api/v1/funnels/{funnelId}/steps/{stepId}` and use `viewUrl` to inspect the result.

5. Build a sales Funnel with Order bump, upsell, and thank-you Funnel Step.

   - Create each Product: `POST /api/v1/products` with top-level fields such as `{ "name": "Core Course", "value": 97, "status": "publish" }`.
   - Create each Offer: `POST /api/v1/offers` with `{ "name": "Core Offer", "price": 97, "active": true, "products": [{"productId": productId}] }`.
   - Create the Funnel and Funnel Steps:
     1. `POST /api/v1/funnels` for the Funnel.
     2. `POST /api/v1/funnels/{funnelId}/steps` for Checkout.
     3. `POST /api/v1/funnels/{funnelId}/steps` for Upsell.
     4. `POST /api/v1/funnels/{funnelId}/steps` for Thank You.
   - Attach the main Offer and bump to Checkout:

     ```json
     {"offers": [{"offerId": 123}], "bumps": [{"offerId": 456}]}
     ```

     Send it to `POST /api/v1/funnels/{funnelId}/steps/{checkoutStepId}`.
   - Attach the upsell Offer to the Upsell Funnel Step with `POST /api/v1/funnels/{funnelId}/steps/{upsellStepId}` and `{ "offers": [{"offerId": upsellOfferId}] }`.
   - Replace the Checkout Design with a `checkout` element and `checkout` behavior. Replace the Upsell Design with a `checkout-offer` element for the upsell Offer.

6. Build a Dent-selling Funnel that provisions a Tenant.

   - Create a Webhook Endpoint for Platform provisioning. `customHeaders` is write-only; use it to send the Platform bearer token:

     ```json
     {
       "name": "Platform Provisioning",
       "url": "https://dent.lndo.site/platform/api/v1/provisioning",
       "events": [],
       "customHeaders": {"Authorization": "Bearer <platform-token>"},
       "enabled": true
     }
     ```

     Send it to `POST /api/v1/webhook-endpoints`.
   - Create the Dent Product with a delivery webhook that sends Platform provisioning data:

     ```json
     {
       "name": "Dent Pro",
       "value": 197,
       "status": "publish",
       "webhooks": [{
         "endpointId": 17,
         "parameters": {
           "email": "{{ contact.email }}",
           "first_name": "{{ contact.firstName }}",
           "last_name": "{{ contact.lastName }}",
           "plan": "pro",
           "order_id": "{{ order.id }}",
           "source": "dent-selling-funnel",
           "slug": "buyer-tenant-slug",
           "name": "Buyer Tenant"
         }
       }]
     }
     ```

     Send it to `POST /api/v1/products`.
   - Create the Offer for that Product, then build Checkout, Upsell, and Thank You Funnel Steps exactly as in step 5.
   - A TestGateway purchase of the Offer creates the Order, runs Product delivery, posts `product.delivery` to Platform provisioning, records a delivered Webhook Delivery with `responseCode: 200`, and creates a Tenant with `plan: "pro"`, `status: "active"`, and `setupStatus: "ready"`.

7. Create Articles and Pages as Designables.

   - Article: `POST /api/v1/articles` with top-level `{ "title", "status", "excerpt", "slug", "design" }`. Dent renders the Article content from the Design.
   - Page: `POST /api/v1/pages` with `{ "title", "status", "slug" }`, then `POST /api/v1/pages/{pageId}/replace-design` with `{ "design": {"version": 1, "elements": [...] } }`.
   - Read the saved Design with `GET /api/v1/pages/{pageId}/design` or `GET /api/v1/articles/{articleId}`.
   - A Funnel Step is not a Page. Never call a Funnel Step a Page.

8. Iterate on Funnel Step Design until the operator approves.

   - Read the current Design: `GET /api/v1/funnels/{funnelId}/steps/{stepId}/design`.
   - Replace it: `POST /api/v1/funnels/{funnelId}/steps/{stepId}/replace-design` with `{ "design": ... }`.
   - Read the Funnel Step: `GET /api/v1/funnels/{funnelId}/steps/{stepId}`.
   - Open or refresh `viewUrl`, inspect the rendered Site result, then repeat. If a render needs WordPress-only admin routes, stop and report the gap instead of using them.

9. Create Components and Templates.

   - Template create works first-party: `POST /api/v1/templates` with `{ "name", "type": "single", "entityType": "page", "status": "publish", "design" }`.
   - Read Template Design with `GET /api/v1/templates/{templateId}/design`.
   - Component create works first-party: `POST /api/v1/components` with `{ "key", "name", "status": "publish", "design" }`.
   - Component Design uses variants:

     ```json
     {
       "design": {
         "version": 1,
         "defaultVariant": "base",
         "properties": [],
         "slots": [],
         "variants": [{
           "key": "base",
           "name": "Base",
           "elements": [{"key": "component-root", "type": "section", "children": []}]
         }]
       }
     }
     ```

   - Read Component Design with `GET /api/v1/components/{componentId}/design`.
   - Replace Component Design with `POST /api/v1/components/{componentId}/replace-design`.

10. Query Analytics, break down revenue, and export CSV.

   - Use `GET /api/v1/dent/breakdown?dimension=source&period=last_month&includeAdmin=true&limit=5` for breakdowns.
   - Use `GET /api/v1/orders/revenue-by-source?period=last_month&attribution=first_touch` for source revenue.
   - Use `GET /api/v1/dent/export-breakdown?dimension=source&period=last_month&includeAdmin=true&limit=1000` with `Accept: text/csv` for scripted analysis. The response is CSV, not JSON.

11. Set up selling: Products, Offers, Courses, Spaces.

   - Course: `POST /api/v1/courses` with `{ "title", "slug", "description", "status": "published", "privacy": "secret" }`.
   - Product that grants a Course: `POST /api/v1/products` with `{ "name", "value", "status": "publish", "courseIds": [courseId] }`.
   - Product that grants a Space: use `spaceIds: [spaceId]`.
   - Offer: `POST /api/v1/offers` with `{ "name", "price", "active": true, "products": [{"productId": productId}] }`.
   - Users buy Offers. Products are what Offers include.

12. Add Course content and manage Spaces.

   - Section: `POST /api/v1/courses/{courseId}/sections` with `{ "title", "position", "status": "published" }`.
   - Read ordered Course Sections with `GET /api/v1/courses/{courseId}/sections`.
   - Lesson: `POST /api/v1/sections/{sectionId}/lessons` with `{ "title", "body", "position", "status": "published", "contentType": "text" }`.
   - Read ordered Section Lessons with `GET /api/v1/sections/{sectionId}/lessons`.
   - Space: `POST /api/v1/spaces` with `{ "title", "slug", "privacy": "secret", "status": "published" }`.
   - Contacts are created by identify flows such as opt-in forms and Checkout email capture; Accounts do not author Contacts directly.
   - Add an existing Contact to a Space: `POST /api/v1/spaces/{spaceId}/add-member` with `{ "contact_id": contactId, "role": "member" }`.
   - Read Space members: `GET /api/v1/spaces/{spaceId}/list-members`.

13. Preserve correctness while operating.

   - Use Dent's words exactly; never write Product when the object is an Offer, never write Page when the object is a Funnel Step.
   - Prefer small read-after-write checks and include exact ids changed in your answer.
   - Show Dent's error status and body. Do not invent fallbacks.
   - Do not use `/wp/wp-login.php`, WordPress admin-ajax, Bricks save routes, or WordPress REST routes for these workflows. A need for those routes is a first-party API gap to report.
   - Never place a bearer token in argv, a project file, a report, or a commit.

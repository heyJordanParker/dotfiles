---
name: dent
version: 0.1.5
description: Operate Dent via first-party API. Trigger: Dent API, Funnels, Offers, Pages, Articles, Courses, Spaces, Analytics. Do not trigger for WordPress-only work.
---

# Dent

Dent spins up marketing Sites that sell and deliver digital Products. Operate it through the first-party schema-driven API. Vocabulary is strict: Funnel, Funnel Step, Offer, Product, Order, Cart, Checkout, Journey, Experiment, Conversion, Link, Session, Visitor Event, Attribution, Designer, Design, Designable, Site, Page, Article, Space, Course.

## Process

1. Establish the session before touching data.

   - IF `dent` is not on PATH, run each Dent CLI command as `npx @parkerlabs/dent@latest <command>`.
   - Run the local checks:

     ```bash
     dent check
     dent whoami
     ```

   - If the session-start check or `dent check` says the skill is stale, update the skill before a write. Use the update reference instead of asking the user to touch a terminal.
   - If `dent whoami` fails, run `dent login`. The standalone CLI starts Dent's OAuth 2.0 Authorization Code + PKCE login against production Dent by default (`https://ondent.app`): it opens the browser, the operator signs in and clicks Authorize, the loopback callback receives the authorization code, and the CLI exchanges code + verifier before saving the token. This is the same flow an agent can guide; the agent layer does not replace the CLI.
   - If loopback cannot reach the CLI (remote browser, web agent, locked-down machine), run `dent login --copy-code`. The operator authorizes in the browser, copies the short code Dent shows, and pastes it into the CLI. The code still exchanges only with that CLI's PKCE verifier.
   - Never ask the operator to create a personal access token for normal login. Never put a token in argv. Explicit token input remains only for power-user recovery with `--token-prompt` or `--stdin`.

     ```bash
     dent login
     dent login --copy-code
     ```

   - Use `dent setup` only when developing against a local Dent repo. It switches the CLI and skill to the local target while preserving the live credential. Use `dent reset` to switch back to the live target without re-authenticating either target.

   - Use `dent logout` when the active target's stored credential should be removed.

   - Power users may set the credential variables below. `DENT_TARGET` selects `live` or `local`; `DENT_SITE_URL` and `DENT_API_KEY` each override the matching active stored credential and fall back to stored config for the other:

      ```bash
      export DENT_TARGET=live
      export DENT_SITE_URL=https://example.com
      export DENT_API_KEY=...
      ```

2. Fetch the schema catalog first; never guess entity names, fields, actions, or routes.

   - Catalog: `GET /api/v1/schema/`.
   - Local development target switching is owned by the CLI's `dent setup` and `dent reset`; do not hardcode local Dent URLs in the skill.
   - Read the target entity's `fields`, `actions`, `parameters`, `mode`, `target`, and `access` before writing. The Funnel Step `status` field is in the catalog; set it through the generic step update route.
   - Action bodies are top-level fields. Do not wrap them in `{attributes: ...}` unless the catalog parameter is literally named `attributes` and the action expects that wrapper from a non-generic endpoint. For Dent generic create/update/actions, send the fields directly.

   ```bash
   dent schema
    dent schema funnels
    dent api funnels
    dent api courses 9203 sections
    dent api dent breakdown --param dimension=source --param period=last_month --param includeAdmin=true --param limit=5
   ```

   `dent api` resolves HTTP methods from the tenant schema catalog for each invocation: `read` actions use `GET`, `write` and `remote` actions use `POST`, and `destroy` actions use `DELETE`. `--method` is the explicit override. Do not infer a write from positional shape; a bare nested collection like `dent api courses 9203 sections` is a list (`GET`). Creates require `--data` or an explicit method.

3. Use the generic route grammar the catalog confirms, and route every Page surface through interview, writing, then design.

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
   - For any Page, Article, Funnel Step, Checkout Funnel Step, Upsell Funnel Step, or redesign, use this exact order:
     1. FIRST establish the brief by interviewing the operator in plain language → `references/interview.md`.
     2. THEN write the finished copy → `references/writing/copywriting.md` plus the matching writing leaf.
     3. THEN design from the brief and finished copy → `references/design/designing.md` plus the matching design leaf.
   - Match the leaf to the surface:
     - Opt-in Page or lead-capture Funnel Step → `references/writing/optin-page.md` and `references/design/optin-page.md`.
     - Sales Page or sales Funnel Step → `references/writing/sales-page.md` and `references/design/sales-page.md`.
     - Checkout Funnel Step → `references/writing/sales-page.md` and the Checkout starter inside `references/design/sales-page.md`.
     - Upsell Funnel Step → `references/writing/upsell-page.md` and `references/design/upsell-page.md`.
     - Thank-you Page or Funnel Step → `references/writing/thank-you-page.md` and `references/design/thank-you-page.md`.
     - Article or content Page → `references/writing/article.md` and `references/design/article.md`.
   - Do not duplicate the reference content inline. Use the references to produce the brief, copy, and Designer JSON, then send the resulting Design through the API owner.

4. Build an opt-in Funnel with a thank-you Funnel Step.

   - Start with the page creation sequence for both public steps:
     - Opt-in Funnel Step → interview brief, opt-in copy, opt-in Design.
     - Thank-you Funnel Step → interview brief, thank-you copy, thank-you Design.
   - Create the Funnel: `POST /api/v1/funnels` with `{ "name": "Lead magnet opt-in" }`.
   - Use `homeStepId` from the response as the opt-in Funnel Step.
   - Create the thank-you Funnel Step: `POST /api/v1/funnels/{funnelId}/steps` with `{ "title": "Thank You" }`.
   - Author the opt-in Design through the Funnel Step, not WordPress. Every `{{ fields.* }}` value used by a behavior must have a matching input whose `config.name` is that field.
   - Send the opt-in Design to `POST /api/v1/funnels/{funnelId}/steps/{stepId}/replace-design`.
   - Put a polished thank-you Design on the thank-you Funnel Step so the public URL is not empty.
   - Send the thank-you Design to `POST /api/v1/funnels/{funnelId}/steps/{thankYouStepId}/replace-design`.
   - Publish every public Funnel Step before opening `viewUrl` or submitting the form:

     ```json
     {"status": "published"}
     ```

     Send it to `POST /api/v1/funnels/{funnelId}/steps/{stepId}` for the opt-in and thank-you Funnel Steps. Public URLs and public form submits require a published Funnel Step on an active Funnel.
   - Read the Funnel Step with `GET /api/v1/funnels/{funnelId}/steps/{stepId}` after `replace-design` and publishing. Confirm `status: "published"`, `viewUrl`, `hasOptinAction: true` for an opt-in step, and `hasBuilder` before opening the public URL. `replace-design` returns the saved Design, not the Step metadata.
   - Verify the public opt-in path with a real pageview and form submit:
     1. `GET` the opt-in Step `viewUrl` with a cookie jar to render the form and establish the Visitor Session.
     2. `POST /api/v1/forms/submit` with the same cookies and body `{ "owner": {"type": "step", "id": stepId}, "form": "optin-form", "fields": {"email": "visitor@example.com", "name": "Visitor Name"} }`.
     3. Treat `success: true` and the returned `redirectUrl` as the submit proof; follow `redirectUrl` when the form redirects.
     4. Find the captured Contact with `GET /api/v1/contacts?search=visitor@example.com`.

5. Build a sales Funnel with Order bump, upsell, and thank-you Funnel Step.

   - Start with the page creation sequence for each public step:
     - Sales or Checkout Funnel Step → interview brief, sales/Checkout copy, sales or Checkout Design.
     - Upsell Funnel Step → interview brief, upsell copy, upsell Design.
     - Thank-you Funnel Step → interview brief, thank-you copy, thank-you Design.
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
   - Send each finished Design to its owner:
     - Checkout Design → `POST /api/v1/funnels/{funnelId}/steps/{checkoutStepId}/replace-design`.
     - Upsell Design → `POST /api/v1/funnels/{funnelId}/steps/{upsellStepId}/replace-design`.
     - Thank-you Design → `POST /api/v1/funnels/{funnelId}/steps/{thankYouStepId}/replace-design`.
   - Publish every public Funnel Step before opening `viewUrl` or submitting Checkout:

     ```json
     {"status": "published"}
     ```

     Send it to `POST /api/v1/funnels/{funnelId}/steps/{stepId}` for the Checkout, Upsell, and Thank You Funnel Steps. Public URLs, form submits, and Checkout require a published Funnel Step on an active Funnel.
   - Start Checkout like a real Visitor: `GET` the Checkout Funnel Step `viewUrl` first and keep the response cookies. That pageview creates the Funnel Session/Journey/Cart context. Send later `POST /api/v1/funnel/cart` mutations and Checkout submit calls with those same cookies; without the step pageview they return `401 No active funnel session`.

6. Build a Dent-selling Funnel that delivers Dent access and triggers tenant provisioning.

   - Start with the page creation sequence for the Dent-selling Checkout, Upsell, and Thank You Funnel Steps so the Funnel presents the Offer like a polished sales surface, not a wireframe.
   - Provisioning is Product delivery. Configure the Product included in the sold Offer with the access grants and provisioning webhook it must deliver after purchase; do not add a separate operator API step.
   - Create or choose the Webhook Endpoint for the provisioning webhook target. Dent signs every delivery with an HMAC secret it generates on create and returns once in the create response; capture that `secret` and verify the signature on the target. `customHeaders` is not a create field — it is not mass-assignable, so sending it in the body is silently dropped:

     ```json
     {
       "name": "Dent Provisioning",
       "url": "https://provisioning.example.com/dent/product-delivery",
       "events": [],
       "enabled": true
     }
     ```

     Send it to `POST /api/v1/webhook-endpoints` and read `secret` from the response.
   - Create the Dent Product with Product delivery configuration. Use `courseIds` or `spaceIds` for Dent access grants the buyer receives, and `webhooks` for the provisioning webhook payload:

     ```json
     {
       "name": "Dent Pro",
       "value": 197,
       "status": "publish",
       "courseIds": [321],
       "spaceIds": [654],
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
   - Publish every public Funnel Step exactly as in step 5 before opening `viewUrl`, changing the Cart, or submitting Checkout.
   - Before Cart or Checkout API calls, `GET` the Checkout Funnel Step `viewUrl` with a cookie jar and reuse those cookies for `POST /api/v1/funnel/cart` and Checkout submit calls.
   - A TestGateway purchase of the Offer creates the Order, runs Product delivery, grants the configured Course or Space access, posts `product.delivery` to the configured provisioning webhook, and records the Webhook Delivery response. Treat tenant creation as the provisioning webhook's downstream effect, not as a separate operator API step in this skill.

7. Create Articles and Pages as Designables.

   - Start with the page creation sequence before writing the `design` field or calling `replace-design`:
     - Article or content Page → interview brief, Article/content copy, Article/content Design.
     - Opt-in Page → interview brief, opt-in copy, opt-in Design.
     - Sales Page → interview brief, sales copy, sales Design.
     - Thank-you Page → interview brief, thank-you copy, thank-you Design.
   - Article: `POST /api/v1/articles` with top-level `{ "title", "status", "excerpt", "slug", "design" }`. Dent renders the Article content from the Design.
   - Page: `POST /api/v1/pages` with `{ "title", "status", "slug" }`, then `POST /api/v1/pages/{pageId}/replace-design` with `{ "design": {"version": 1, "elements": [...] } }`.
   - Read the saved Design with `GET /api/v1/pages/{pageId}/design` or `GET /api/v1/articles/{articleId}`.
   - A Funnel Step is not a Page. Never call a Funnel Step a Page.

8. Iterate on Funnel Step Design until the operator approves.

   - Read the current Design: `GET /api/v1/funnels/{funnelId}/steps/{stepId}/design`.
   - Re-establish or update the brief by interviewing the operator in plain language about what the current Step must do differently.
   - Rewrite or confirm the finished copy through `references/writing/copywriting.md` and the matching writing leaf before changing layout.
   - Redesign from the updated brief and finished copy through `references/design/designing.md` and the matching design leaf.
   - Replace it: `POST /api/v1/funnels/{funnelId}/steps/{stepId}/replace-design` with `{ "design": ... }`.
   - If the operator will inspect the public `viewUrl`, publish the Funnel Step first with `POST /api/v1/funnels/{funnelId}/steps/{stepId}` and `{ "status": "published" }`.
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

10. Query Analytics and break down revenue.

   - Use `GET /api/v1/dent/breakdown?dimension=source&period=last_month&includeAdmin=true&limit=5` for breakdowns.
   - Use `GET /api/v1/orders/revenue-by-source?period=last_month&attribution=first_touch` for source revenue.
   - `breakdown` accepts `dimension`. Available dimensions: `source`, `referrer`, `campaign`, `country`, `region`, `city`, `device`, `browser`, `entry_pages`.
   - In the CLI variant, pass catalog action parameters as query parameters:

     ```bash
     dent api dent breakdown --param dimension=source --param period=last_month --param includeAdmin=true --param limit=5
     ```

11. Set up selling: Products, Offers, Courses, Spaces.

   - Course: `POST /api/v1/courses` with `{ "title", "slug", "description", "status": "published", "privacy": "secret" }`.
   - Product that grants a Course: `POST /api/v1/products` with `{ "name", "value", "status": "publish", "courseIds": [courseId] }`.
   - Product that grants a Space: use `spaceIds: [spaceId]`.
   - Offer: `POST /api/v1/offers` with `{ "name", "price", "active": true, "products": [{"productId": productId}] }`.
   - Users buy Offers. Products are what Offers include.

12. Add Course content and manage Spaces.

   - Section: `POST /api/v1/courses/{courseId}/sections` with `{ "title", "position" }`. Sections publish by default; `status` is not a create field, so do not send it.
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
   - Publishing a Funnel Step or Page (`{"status": "published"}`), or `replace-design` on one already published, validates the Design at the persistence chokepoint. If publish validation fails — invalid expression syntax, incomplete forms, or unresolved script or branch references — Dent returns `422` with element-keyed messages and does not persist. Fix the Design, not the status call. A draft save keeps its warnings and always persists.
   - Show Dent's error status and body. Do not invent fallbacks.
   - Do not use `/wp/wp-login.php`, WordPress admin-ajax, Bricks save routes, or WordPress REST routes for these workflows. A need for those routes is a first-party API gap to report.
   - Never place a bearer token in argv, a project file, a report, or a commit.

## References (each solves one problem)

- The installed Dent skill is stale → references/updating-the-skill.md
- Checking Dent API route grammar and auth details → references/api.md
- Establishing the Page, Article, or Funnel Step brief in operator language → references/interview.md
- Writing Page, Article, and Funnel Step copy before design → references/writing/copywriting.md
- Writing opt-in Page or lead-capture Funnel Step copy → references/writing/optin-page.md
- Writing sales Page, sales Funnel Step, or Checkout copy → references/writing/sales-page.md
- Writing one-click upsell Funnel Step copy → references/writing/upsell-page.md
- Writing thank-you Page or confirmation Funnel Step copy → references/writing/thank-you-page.md
- Writing Article or content Page copy → references/writing/article.md
- Designing Page, Article, and Funnel Step visuals after copy → references/design/designing.md
- Designing opt-in Page or lead-capture Funnel Step visuals → references/design/optin-page.md
- Designing sales Page, sales Funnel Step, or Checkout visuals → references/design/sales-page.md
- Designing one-click upsell Funnel Step visuals → references/design/upsell-page.md
- Designing thank-you Page or confirmation Funnel Step visuals → references/design/thank-you-page.md
- Designing Article or content Page visuals → references/design/article.md

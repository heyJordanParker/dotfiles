# Modeling translation repairs

Use this Reference when the Affordance tables exist but do not translate cleanly into Database Schema, User Experience, and Architecture.

## 1. Pick the source pattern

### Existing systems start from entry points
For multiple entry points, model each entry point that writes or reads the same Data Store, then render the shared Data Store once.
Example: `admin_organisation_countries` is modified by SSO Admin manual save, a signal handler, and `manage.py dwbn_cleanup`; it is read by DWConnect, an external API call, and a system email field.

### Shaped parts start from Rs and reusable patterns
For new work, copy the Rs from Shaping, name the existing parts being adapted, and translate each new part into UI Affordances, Code Affordances, and Data Stores.
Example: letter search adapts URL state, search input, data fetching, pagination, and rendering from an existing global search page.

## 2. Render Database Schema from the stores

### Persistent stores become schema entries
Database tables, fields, model methods, and migration changes belong in Database Schema.
Example: `role_profiles`, `admin_organisation_countries`, and `organisations` render as admin tables.

### Component state stays out of persistent schema
If no persistent model changes, say so and list the state that matters to the User path.
Example: `letter-browser` state lists `loading`, `detailResult`, `activeQuery`, `compact`, `parentId`, and `fullPageRoute`.

## 3. Render User Experience from UI Affordances

### Group by Critical Path and Place
Show what the User sees, then inline the Wires Out and Returns To that explain behavior.
Example: SSO Admin shows role checkboxes, the superuser-only admin countries fieldset, Add and Remove buttons, and Save wiring to `save_form()`.
Example: Letter search shows search input, loading spinner, no-results message, result count, result rows, scroll pagination, and See All navigation.

### External User-visible outputs stay visible
If the User sees an email, notification, export, or external page, include it in User Experience even when no page component owns it.
Example: system email From field reads `admin_organisation_countries`.

## 4. Render Architecture from Code Affordances

### Name writers, readers, and signal chains
Architecture must show each public API, scheduled entry point, signal, and dependency that moves data across a boundary.
Example: `save_form()` calls form save and `_update_user_m2m()`, which emits `user_m2m_field_updated`, which reaches the `sso-dwbn-theme` handler.

### Name adapted Precedent
For new components, name the existing system being adapted and list the public methods or inputs that form the contract.
Example: `letter-browser` builds on the global search page through `activeQuery`, `performSearch()`, `appendNextPage()`, `initializeState()`, `typesense.service`, `intercom.service`, and Router.

## 5. Reconcile the count

### Count the same Affordances in tables and presentation
The Architect-facing sections must not drop or duplicate table rows.
Example: existing system mapping has 9 UI Affordances, 17 Code Affordances, and 3 Data Stores; 29 Affordances appear in the presentation.
Example: shaped letter search has 14 UI Affordances and 18 Code Affordances; 32 Affordances appear in the presentation.

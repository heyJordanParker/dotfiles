# Casing Reference

Follow the project first. If no project convention exists, follow the ecosystem.

## By Language/Context

**TypeScript/JavaScript**
- Variables/functions: camelCase
- Classes/components/types: PascalCase

**PHP**
- Classes: PascalCase
- Methods/variables: camelCase
- Database tables/columns: snake_case
- Route parameters: snake_case

**Python**
- Almost everything: snake_case
- Classes: PascalCase

**CSS**
- Custom stylesheets: BEM (`block__element--modifier`)
- Tailwind/utility classes: kebab-case

**HTML**
- React: camelCase attributes (`className`, `onClick`)
- Vanilla HTML: lowercase

**URLs/Routes**
- kebab-case or snake_case
- Use routing to improve names (URL is user-facing)

**Databases**
- Tables: snake_case, plural (`users`, `order_items`)
- Columns: snake_case (`created_at`, `user_id`)
- Constraints: snake_case

## When Conventions Conflict

The outer context wins. A TypeScript API response from a PHP backend keeps snake_case keys - don't transform just to match JS convention.

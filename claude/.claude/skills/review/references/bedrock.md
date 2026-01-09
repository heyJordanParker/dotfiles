# Bedrock Stack Patterns

Anti-slop patterns for Laravel, WordPress, Acorn, Sage, and Radicle.

## Framework Bypass

**Slop:** Reinventing what the framework provides.

- Raw `foreach` + conditionals → `collect()->filter()->map()`
- `get_posts()` in controllers → Eloquent models: `Post::published()->get()`
- `get_post_meta()` directly → `$post->getMeta('key', 'default')`
- Manual array manipulation → Laravel Collection methods
- Custom validation logic → Laravel validation rules
- `$_GET`/`$_POST`/`$_REQUEST` → `$request->input()`

## Configuration

**Slop:** Hardcoded values that should be configurable.

```php
// Slop
$apiKey = 'sk-abc123';
$endpoint = 'https://api.example.com';

// Clean
$apiKey = config('services.example.key');
$endpoint = config('services.example.endpoint');
```

- API keys, secrets → `.env` + `config()`
- URLs, endpoints → `.env` + `config()`
- Magic numbers with meaning → named constants or config
- `env()` in code → `config()` (for cacheability)

## Paths and URLs

**Slop:** Hardcoded paths that break across environments.

- `'/wp-content/themes/...'` → `get_theme_file_uri()`
- `'/wp-content/uploads/...'` → `wp_upload_dir()['baseurl']`
- `'https://mysite.com/...'` → `home_url()`, `site_url()`
- `'/app/public/...'` → `public_path()`, `asset()`
- Hardcoded admin URLs → `admin_url()`

## Templates

**Slop:** Raw PHP in Blade templates.

```php
// Slop - raw PHP block
<?php
$items = get_items();
foreach ($items as $item) {
    echo $item->name;
}
?>

// Clean - Blade directives
@foreach($items as $item)
  {{ $item->name }}
@endforeach
```

- `<?php ?>` blocks → Blade directives
- `echo` statements → `{{ }}` or `{!! !!}`
- Complex logic in templates → Move to controller/composer

## Output Escaping

**Slop:** Unescaped output that could contain user data.

```php
// WordPress - Slop
echo $user_input;
echo $title;

// WordPress - Clean
echo esc_html($user_input);
echo esc_attr($attribute);
echo wp_kses_post($html_content);

// Blade - auto-escapes
{{ $user_input }}  // Safe - escaped

// Blade - raw (only for trusted HTML)
{!! $trusted_html !!}  // Dangerous if not trusted
```

**Rule:** `{!! !!}` only for HTML you control. Never for user input.

## Service Container

**Slop:** Manual instantiation everywhere.

```php
// Slop
$service = new PaymentService(new Logger(), new Config());

// Clean - injection
public function __construct(
    protected PaymentService $payments
) {}

// Clean - resolution
$service = app(PaymentService::class);
```

Use container for services with dependencies. Direct `new` is fine for simple value objects.

## PHP Style (Radicle)

**Slop:** Outdated PHP patterns.

- `$snake_case` variables → `$camelCase`
- `'value' === $var` (Yoda) → `$var === 'value'`
- `array()` → `[]`
- `if/else` chains → Early returns, guard clauses
- No type declarations → `function name(Type $param): ReturnType`
- `$this->prop = $prop` in constructor → Constructor promotion: `protected Type $prop`

## REST API

**Slop:** Insecure or incomplete API registration.

```php
// Slop - missing validation and auth
register_rest_route('api/v1', '/items', [
    'methods' => 'POST',
    'callback' => [$this, 'createItem'],
]);

// Clean
register_rest_route('api/v1', '/items', [
    'methods' => 'POST',
    'callback' => [$this, 'createItem'],
    'permission_callback' => [$this, 'canCreateItems'],
    'args' => [
        'name' => [
            'required' => true,
            'type' => 'string',
            'sanitize_callback' => 'sanitize_text_field',
        ],
    ],
]);
```

Required:
- `permission_callback` – Always. Use `'__return_true'` only for truly public endpoints.
- `args` with validation – For any input parameters.
- `WP_REST_Response` – Not bare arrays.
- `WP_Error` – For error responses with proper status codes.

## AJAX

**Slop:** Legacy WordPress AJAX.

```php
// Slop - admin-ajax.php
add_action('wp_ajax_my_action', 'handle_my_action');
add_action('wp_ajax_nopriv_my_action', 'handle_my_action');

// Clean - REST API
register_rest_route('api/v1', '/my-endpoint', [...]);
```

Always use REST API. Never `admin-ajax.php` for new code.

## Script Data

**Slop:** Deprecated localization pattern.

```php
// Slop
wp_localize_script('my-script', 'myData', ['key' => 'value']);

// Clean
wp_add_inline_script(
    'my-script',
    'window.myData = ' . wp_json_encode(['key' => 'value']) . ';',
    'before'
);
```

## Hooks in Providers

**Slop:** Business logic in service providers.

```php
// Slop - logic in provider
public function boot(): void
{
    add_action('init', function () {
        $items = Item::all();
        foreach ($items as $item) {
            // complex processing...
        }
    });
}

// Clean - delegate to service
public function boot(): void
{
    add_action('init', [app(ItemProcessor::class), 'process']);
}
```

- `register()` – Container bindings only
- `boot()` – Hook registration only, delegate logic to services

## Blocks

**Slop:** Client-side rendering when server-side is better.

```jsx
// Slop - JSX in save()
save: ({ attributes }) => {
    return <div className="my-block">{attributes.content}</div>;
}

// Clean - server render
save: () => null,  // Render via Blade template
```

For dynamic content, use `save: () => null` with PHP/Blade rendering.

## Block Attributes

**Slop:** Passing attributes to views unsanitized.

```php
// Slop
return view('blocks.my-block', ['content' => $attributes['content']]);

// Clean
return view('blocks.my-block', [
    'content' => wp_kses_post($attributes['content']),
    'title' => sanitize_text_field($attributes['title']),
]);
```

## Quick Reference

- `get_posts()` in controller → Use Eloquent model
- Hardcoded URL/path → Use helper function
- `<?php ?>` in Blade → Use directives
- `echo $var` in WP → `echo esc_html($var)`
- `{!! $user_input !!}` → `{{ $user_input }}`
- `new Service()` everywhere → Use container
- `$snake_case` → `$camelCase`
- `wp_ajax_*` hooks → REST API
- `wp_localize_script()` → `wp_add_inline_script()`
- Logic in provider → Delegate to service
- JSX in block `save()` → Server-side render

# Database Performance Best Practices

## Always Eager Load Relationships

Lazy loading causes N+1 query problems — one query per loop iteration. Always use `with()` to load relationships upfront.

Incorrect (N+1 — executes 1 + N queries):
```php
$posts = Post::all();
foreach ($posts as $post) {
    echo $post->author->name;
}
```

Correct (2 queries total):
```php
$posts = Post::with('author')->get();
foreach ($posts as $post) {
    echo $post->author->name;
}
```

Constrain eager loads to select only needed columns (always include the foreign key):

```php
$users = User::with(['posts' => function ($query) {
    $query->select('id', 'user_id', 'title')
          ->where('published', true)
          ->latest()
          ->limit(10);
}])->get();
```

## Prevent Lazy Loading in Development

Enable this in `AppServiceProvider::boot()` to catch N+1 issues during development.

```php
public function boot(): void
{
    Model::preventLazyLoading(! app()->isProduction());
}
```

Throws `LazyLoadingViolationException` when a relationship is accessed without being eager-loaded.

## Select Only Needed Columns

Avoid `SELECT *` — especially when tables have large text or JSON columns.

Incorrect:
```php
$posts = Post::with('author')->get();
```

Correct:
```php
$posts = Post::select('id', 'title', 'user_id', 'created_at')
    ->with(['author:id,name,avatar'])
    ->get();
```

When selecting columns on eager-loaded relationships, always include the foreign key column or the relationship won't match.

## Chunk Large Datasets

Never load thousands of records at once. Use chunking for batch processing.

Incorrect:
```php
$users = User::all();
foreach ($users as $user) {
    $user->notify(new WeeklyDigest);
}
```

Correct:
```php
User::where('subscribed', true)->chunk(200, function ($users) {
    foreach ($users as $user) {
        $user->notify(new WeeklyDigest);
    }
});
```

Use `chunkById()` when modifying records during iteration — standard `chunk()` uses OFFSET which shifts when rows change:

```php
User::where('active', false)->chunkById(200, function ($users) {
    $users->each->delete();
});
```

## Add Database Indexes

Index columns that appear in `WHERE`, `ORDER BY`, `JOIN`, and `GROUP BY` clauses.

Incorrect:
```php
Schema::create('orders', function (Blueprint $table) {
    $table->id();
    $table->foreignId('user_id')->constrained();
    $table->string('status');
    $table->timestamps();
});
```

Correct:
```php
Schema::create('orders', function (Blueprint $table) {
    $table->id();
    $table->foreignId('user_id')->index()->constrained();
    $table->string('status')->index();
    $table->timestamps();
    $table->index(['status', 'created_at']);
});
```

Add composite indexes for common query patterns (e.g., `WHERE status = ? ORDER BY created_at`).

## Use `withCount()` for Counting Relations

Never load entire collections just to count them.

Incorrect:
```php
$posts = Post::all();
foreach ($posts as $post) {
    echo $post->comments->count();
}
```

Correct:
```php
$posts = Post::withCount('comments')->get();
foreach ($posts as $post) {
    echo $post->comments_count;
}
```

Conditional counting:

```php
$posts = Post::withCount([
    'comments',
    'comments as approved_comments_count' => function ($query) {
        $query->where('approved', true);
    },
])->get();
```

## Use `cursor()` for Memory-Efficient Iteration

For read-only iteration over large result sets, `cursor()` loads one record at a time via a PHP generator.

Incorrect:
```php
$users = User::where('active', true)->get();
```

Correct:
```php
foreach (User::where('active', true)->cursor() as $user) {
    ProcessUser::dispatch($user->id);
}
```

Use `cursor()` for read-only iteration. Use `chunk()` / `chunkById()` when modifying records.

## No Queries in Blade Templates

Never execute queries in Blade templates. Pass data from controllers.

Incorrect:
```blade
@foreach (User::all() as $user)
    {{ $user->profile->name }}
@endforeach
```

Correct:
```php
// Controller
$users = User::with('profile')->get();
return view('users.index', compact('users'));
```

```blade
@foreach ($users as $user)
    {{ $user->profile->name }}
@endforeach
```

# Advanced Query Patterns

## Use `addSelect()` Subqueries for Single Values from Has-Many

Instead of eager-loading an entire has-many relationship for a single value (like the latest timestamp), use a correlated subquery via `addSelect()`. This pulls the value directly in the main SQL query — zero extra queries.

```php
public function scopeWithLastLoginAt($query): void
{
    $query->addSelect([
        'last_login_at' => Login::select('created_at')
            ->whereColumn('user_id', 'users.id')
            ->latest()
            ->take(1),
    ])->withCasts(['last_login_at' => 'datetime']);
}
```

## Create Dynamic Relationships via Subquery FK

Extend the `addSelect()` pattern to fetch a foreign key via subquery, then define a `belongsTo` relationship on that virtual attribute. This provides a fully-hydrated related model without loading the entire collection.

```php
public function lastLogin(): BelongsTo
{
    return $this->belongsTo(Login::class);
}

public function scopeWithLastLogin($query): void
{
    $query->addSelect([
        'last_login_id' => Login::select('id')
            ->whereColumn('user_id', 'users.id')
            ->latest()
            ->take(1),
    ])->with('lastLogin');
}
```

## Use Conditional Aggregates Instead of Multiple Count Queries

Replace N separate `count()` queries with a single query using `CASE WHEN` inside `selectRaw()`. Use `toBase()` to skip model hydration when you only need scalar values.

```php
$statuses = Feature::toBase()
    ->selectRaw("count(case when status = 'Requested' then 1 end) as requested")
    ->selectRaw("count(case when status = 'Planned' then 1 end) as planned")
    ->selectRaw("count(case when status = 'Completed' then 1 end) as completed")
    ->first();
```

## Use `setRelation()` to Prevent Circular N+1

When a parent model is eager-loaded with its children, and the view also needs `$child->parent`, use `setRelation()` to inject the already-loaded parent rather than letting Eloquent fire N additional queries.

```php
$feature->load('comments.user');
$feature->comments->each->setRelation('feature', $feature);
```

## Prefer `whereIn` + Subquery Over `whereHas`

`whereHas()` emits a correlated `EXISTS` subquery that re-executes per row. Using `whereIn()` with a `select('id')` subquery lets the database use an index lookup instead, without loading data into PHP memory.

Incorrect (correlated EXISTS re-executes per row):

```php
$query->whereHas('company', fn ($q) => $q->where('name', 'like', $term));
```

Correct (index-friendly subquery, no PHP memory overhead):

```php
$query->whereIn('company_id', Company::where('name', 'like', $term)->select('id'));
```

## Sometimes Two Simple Queries Beat One Complex Query

Running a small, targeted secondary query and passing its results via `whereIn` is often faster than a single complex correlated subquery or join. The additional round-trip is worthwhile when the secondary query is highly selective and uses its own index.

## Use Compound Indexes Matching `orderBy` Column Order

When ordering by multiple columns, create a single compound index in the same column order as the `ORDER BY` clause. Individual single-column indexes cannot combine for multi-column sorts — the database will filesort without a compound index.

```php
// Migration
$table->index(['last_name', 'first_name']);

// Query — column order must match the index
User::query()->orderBy('last_name')->orderBy('first_name')->paginate();
```

## Use Correlated Subqueries for Has-Many Ordering

When sorting by a value from a has-many relationship, avoid joins (they duplicate rows). Use a correlated subquery inside `orderBy()` instead, paired with an `addSelect` scope for eager loading.

```php
public function scopeOrderByLastLogin($query): void
{
    $query->orderByDesc(Login::select('created_at')
        ->whereColumn('user_id', 'users.id')
        ->latest()
        ->take(1)
    );
}
```

# Collection Best Practices

## Use Higher-Order Messages for Simple Operations

Incorrect:
```php
$users->each(function (User $user) {
    $user->markAsVip();
});
```

Correct: `$users->each->markAsVip();`

Works with `each`, `map`, `sum`, `filter`, `reject`, `contains`, etc.

## Choose `cursor()` vs. `lazy()` Correctly

- `cursor()` — one model in memory, but cannot eager-load relationships (N+1 risk).
- `lazy()` — chunked pagination returning a flat LazyCollection, supports eager loading.

Incorrect: `User::with('roles')->cursor()` — eager loading silently ignored.

Correct: `User::with('roles')->lazy()` for relationship access; `User::cursor()` for attribute-only work.

## Use `lazyById()` When Updating Records While Iterating

`lazy()` uses offset pagination — updating records during iteration can skip or double-process. `lazyById()` uses `id > last_id`, safe against mutation.

## Use `toQuery()` for Bulk Operations on Collections

Avoids manual `whereIn` construction.

Incorrect: `User::whereIn('id', $users->pluck('id'))->update([...]);`

Correct: `$users->toQuery()->update([...]);`

## Use `#[CollectedBy]` for Custom Collection Classes

More declarative than overriding `newCollection()`.

```php
#[CollectedBy(UserCollection::class)]
class User extends Model {}
```
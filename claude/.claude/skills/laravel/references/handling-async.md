# Queue & Job Best Practices

## Set `retry_after` Greater Than `timeout`

If `retry_after` is shorter than the job's `timeout`, the queue worker re-dispatches the job while it's still running, causing duplicate execution.

Incorrect (`retry_after` ≤ `timeout`):
```php
class ProcessReport implements ShouldQueue
{
    public $timeout = 120;
}

// config/queue.php — retry_after: 90 ← job retried while still running!
```

Correct (`retry_after` > `timeout`):
```php
class ProcessReport implements ShouldQueue
{
    public $timeout = 120;
}

// config/queue.php — retry_after: 180 ← safely longer than any job timeout
```

## Use Exponential Backoff

Use progressively longer delays between retries to avoid hammering failing services.

Incorrect (fixed retry interval):
```php
class SyncWithStripe implements ShouldQueue
{
    public $tries = 3;
    // Default: retries immediately, overwhelming the API
}
```

Correct (exponential backoff):
```php
class SyncWithStripe implements ShouldQueue
{
    public $tries = 3;
    public $backoff = [1, 5, 10];
}
```

## Implement `ShouldBeUnique`

Prevent duplicate job processing.

```php
class GenerateInvoice implements ShouldQueue, ShouldBeUnique
{
    public function uniqueId(): string
    {
        return $this->order->id;
    }

    public $uniqueFor = 3600;
}
```

## Always Implement `failed()`

Handle errors explicitly — don't rely on silent failure.

```php
public function failed(?Throwable $exception): void
{
    $this->podcast->update(['status' => 'failed']);
    Log::error('Processing failed', ['id' => $this->podcast->id, 'error' => $exception->getMessage()]);
}
```

## Rate Limit External API Calls in Jobs

Use `RateLimited` middleware to throttle jobs calling third-party APIs.

```php
public function middleware(): array
{
    return [new RateLimited('external-api')];
}
```

## Batch Related Jobs

Use `Bus::batch()` when jobs should succeed or fail together.

```php
Bus::batch([
    new ImportCsvChunk($chunk1),
    new ImportCsvChunk($chunk2),
])
->then(fn (Batch $batch) => Notification::send($user, new ImportComplete))
->catch(fn (Batch $batch, Throwable $e) => Log::error('Batch failed'))
->dispatch();
```

## `retryUntil()` Needs `$tries = 0`

When using time-based retry limits, set `$tries = 0` to avoid premature failure.

```php
public $tries = 0;

public function retryUntil(): DateTime
{
    return now()->addHours(4);
}
```

## Use `WithoutOverlapping::untilProcessing()`

Prevents concurrent execution while allowing new instances to queue.

```php
public function middleware(): array
{
    return [new WithoutOverlapping($this->product->id)->untilProcessing()];
}
```

Without `untilProcessing()`, the lock extends through queue wait time. With it, the lock releases when processing starts.

## Use Horizon for Complex Queue Scenarios

Use Laravel Horizon when you need monitoring, auto-scaling, failure tracking, or multiple queues with different priorities.

```php
// config/horizon.php
'environments' => [
    'production' => [
        'supervisor-1' => [
            'connection' => 'redis',
            'queue' => ['high', 'default', 'low'],
            'balance' => 'auto',
            'minProcesses' => 1,
            'maxProcesses' => 10,
            'tries' => 3,
        ],
    ],
],
```

# Events & Notifications Best Practices

## Rely on Event Discovery

Laravel auto-discovers listeners by reading `handle(EventType $event)` type-hints. No manual registration needed in `AppServiceProvider`.

## Run `event:cache` in Production Deploy

Event discovery scans the filesystem per-request in dev. Cache it in production: `php artisan optimize` or `php artisan event:cache`.

## Use `ShouldDispatchAfterCommit` Inside Transactions

Without it, a queued listener may process before the DB transaction commits, reading data that doesn't exist yet.

```php
class OrderShipped implements ShouldDispatchAfterCommit {}
```

## Always Queue Notifications

Notifications often hit external APIs (email, SMS, Slack). Without `ShouldQueue`, they block the HTTP response.

```php
class InvoicePaid extends Notification implements ShouldQueue
{
    use Queueable;
}
```

## Use `afterCommit()` on Notifications in Transactions

Same race condition as events — the queued notification job may run before the transaction commits.

## Route Notification Channels to Dedicated Queues

Mail and database notifications have different priorities. Use `viaQueues()` to route them to separate queues.

## Use On-Demand Notifications for Non-User Recipients

Avoid creating dummy models to send notifications to arbitrary addresses.

```php
Notification::route('mail', 'admin@example.com')->notify(new SystemAlert());
```

## Implement `HasLocalePreference` on Notifiable Models

Laravel automatically uses the user's preferred locale for all notifications and mailables — no per-call `locale()` needed.

## Implement `ShouldQueue` on the Mailable Class

Makes queueing the default regardless of how the mailable is dispatched. No need to remember `Mail::queue()` at every call site — `Mail::send()` also queues it.

## Use `afterCommit()` on Mailables Inside Transactions

A queued mailable dispatched inside a transaction may process before the commit. Use `$this->afterCommit()` in the constructor.

## Use `assertQueued()` Not `assertSent()` for Queued Mailables

`Mail::assertSent()` only catches synchronous mail. Queued mailables silently pass `assertSent`, giving false confidence.

Incorrect: `Mail::assertSent(OrderShipped::class);` when mailable implements `ShouldQueue`.

Correct: `Mail::assertQueued(OrderShipped::class);`

## Use Markdown Mailables for Transactional Emails

Markdown mailables auto-generate both HTML and plain-text versions, use responsive components, and allow global style customization. Generate with `--markdown` flag.

## Separate Content Tests from Sending Tests

Content tests: instantiate the mailable directly, call `assertSeeInHtml()`.
Sending tests: use `Mail::fake()` and `assertSent()`/`assertQueued()`.
Don't mix them — it conflates concerns and makes tests brittle.

# Task Scheduling Best Practices

## Use `withoutOverlapping()` on Variable-Duration Tasks

Without it, a long-running task spawns a second instance on the next tick, causing double-processing or resource exhaustion.

## Use `onOneServer()` on Multi-Server Deployments

Without it, every server runs the same task simultaneously. Requires a shared cache driver (Redis, database, Memcached).

## Use `runInBackground()` for Concurrent Long Tasks

By default, tasks at the same tick run sequentially. A slow first task delays all subsequent ones. `runInBackground()` runs them as separate processes.

## Use `environments()` to Restrict Tasks

Prevent accidental execution of production-only tasks (billing, reporting) on staging.

```php
Schedule::command('billing:charge')->monthly()->environments(['production']);
```

## Use `takeUntilTimeout()` for Time-Bounded Processing

A task running every 15 minutes that processes an unbounded cursor can overlap with the next run. Bound execution time.

## Use Schedule Groups for Shared Configuration

Avoid repeating `->onOneServer()->timezone('America/New_York')` across many tasks.

```php
Schedule::daily()
    ->onOneServer()
    ->timezone('America/New_York')
    ->group(function () {
        Schedule::command('emails:send --force');
        Schedule::command('emails:prune');
    });
```
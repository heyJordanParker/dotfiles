# React Testing Best Practices

## Test Behavior, Not Implementation

Tests should verify what users see and do, not internal component state. Tests that break during refactoring without behavior changes provide false negatives and no confidence.

```javascript
// Incorrect: Testing implementation details
expect(component.state.isOpen).toBe(true);
expect(wrapper.instance().handleClick).toHaveBeenCalled();

// Correct: Testing visible behavior
await user.click(screen.getByRole('button', { name: /open menu/i }));
expect(screen.getByRole('menu')).toBeVisible();
```

## Follow Query Priority

Prefer queries accessible to everyone. `getByRole` should be the default. `getByTestId` is a last resort because users cannot see or hear test IDs.

```javascript
// Incorrect: Test ID when semantic query works
screen.getByTestId('submit-button')

// Incorrect: Container query bypasses accessibility
container.querySelector('.btn-primary')

// Correct: Role-based query (top priority)
screen.getByRole('button', { name: /submit/i })

// Correct: Label text for form fields
screen.getByLabelText(/email address/i)

// Correct: Text content for non-interactive elements
screen.getByText(/no results found/i)
```

## Use userEvent Over fireEvent

`userEvent` fires complete interaction sequences (keyDown, keyPress, keyUp per character). `fireEvent` dispatches single synthetic events that miss real browser behavior.

```javascript
// Incorrect: Single change event, unrealistic
fireEvent.change(input, { target: { value: 'hello' } });

// Correct: Realistic character-by-character typing
const user = userEvent.setup();
await user.type(screen.getByLabelText(/email/i), 'test@example.com');
await user.click(screen.getByRole('button', { name: /submit/i }));
```

## Choose the Right Query Variant

`getBy` throws on missing elements (assert presence). `queryBy` returns null (assert absence). `findBy` returns a Promise (async elements).

```javascript
// Correct: Asserting element IS present
expect(screen.getByRole('alert')).toBeInTheDocument();

// Correct: Asserting element is NOT present
expect(screen.queryByRole('alert')).not.toBeInTheDocument();

// Correct: Waiting for async element
const alert = await screen.findByRole('alert');
expect(alert).toHaveTextContent(/saved/i);
```

## Mock APIs with MSW at the Network Level

MSW intercepts at the network level, working with any HTTP client. Same mocks serve unit, integration, and E2E tests.

```javascript
// mocks/handlers.js
import { http, HttpResponse } from 'msw';
export const handlers = [
  http.get('/api/users', () => {
    return HttpResponse.json([{ id: 1, name: 'Alice' }]);
  }),
];

// test/setup.ts — wire up server lifecycle
import { setupServer } from 'msw/node';
import { handlers } from './mocks/handlers';
export const server = setupServer(...handlers);
beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

// Per-test override for error states
server.use(
  http.get('/api/users', () => new HttpResponse(null, { status: 500 }))
);
```

## Prefer Integration Tests

The Testing Trophy: integration tests provide the best confidence-per-time ROI. Render real component trees, mock only the network boundary, simulate real user flows. Never test internal state, instance methods, lifecycle calls, or CSS class names.

```javascript
// Correct: Integration test with real component tree
const user = userEvent.setup();
render(<TodoApp />); // TodoForm + TodoList + TodoItem all real
await user.type(screen.getByLabelText(/new todo/i), 'Buy groceries');
await user.click(screen.getByRole('button', { name: /add/i }));
expect(await screen.findByText('Buy groceries')).toBeInTheDocument();
```

## Use findBy for Async, waitFor for Complex Assertions

`findBy` combines `getBy` + `waitFor` and is preferred for single-element async queries. Use `waitFor` only for complex conditions. Never put side effects inside `waitFor`.

```javascript
// Incorrect: waitFor wrapping getBy (redundant)
const btn = await waitFor(() => screen.getByRole('button'));

// Correct: findBy does this internally
const btn = await screen.findByRole('button');

// Incorrect: Side effects inside waitFor (runs multiple times)
await waitFor(() => {
  fireEvent.click(button);
  expect(result).toBeInTheDocument();
});

// Correct: Side effects outside, assertions inside
await user.click(button);
await waitFor(() => expect(result).toBeInTheDocument());
```

## Test Forms, Modals, and Error States

```javascript
// Form: fill fields, submit, verify callback
const handleSubmit = vi.fn();
const user = userEvent.setup();
render(<Form onSubmit={handleSubmit} />);
await user.type(screen.getByLabelText(/name/i), 'Alice');
await user.click(screen.getByRole('button', { name: /submit/i }));
expect(handleSubmit).toHaveBeenCalledWith({ name: 'Alice' });

// Modal: verify open/close with waitForElementToBeRemoved
await user.click(screen.getByRole('button', { name: /open/i }));
expect(screen.getByRole('dialog')).toBeInTheDocument();
await user.click(screen.getByRole('button', { name: /close/i }));
await waitForElementToBeRemoved(() => screen.queryByRole('dialog'));
```

## Use renderHook for Shared Hooks Only

If a hook is used by one component, test it through that component. Use `renderHook` only for shared/reusable hooks.

```javascript
import { renderHook, act } from '@testing-library/react';

test('useCounter increments', () => {
  const { result } = renderHook(() => useCounter());
  expect(result.current.count).toBe(0);
  act(() => result.current.increment());
  expect(result.current.count).toBe(1);
});
```

## Add Accessibility Testing with axe

Use `vitest-axe` (or `jest-axe`) to catch WCAG violations automatically. Use `jsdom` environment (not `happy-dom` due to compatibility issues with axe).

```javascript
import { axe } from 'vitest-axe';

test('form has no accessibility violations', async () => {
  const { container } = render(<LoginForm />);
  expect(await axe(container)).toHaveNoViolations();
});
```

## Configure Vitest for React Testing

Set `environment: 'jsdom'`, `clearMocks: true`, `restoreMocks: true` in vitest config. Setup file should import `@testing-library/jest-dom/vitest` and `vitest-axe/extend-expect`.

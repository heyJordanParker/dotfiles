# Testing

One Process: test User behavior through accessible queries, mock the network edge, and avoid implementation details.

## 1. Test visible behavior

### Test behavior, not implementation

Tests verify what Users see and do, not internal component state. Tests that break during refactoring without behavior changes create false negatives.

Never:
  ```javascript
  expect(component.state.isOpen).toBe(true);
  expect(wrapper.instance().handleClick).toHaveBeenCalled();
  ```

Example:
  ```javascript
  await user.click(screen.getByRole('button', { name: /open menu/i }));
  expect(screen.getByRole('menu')).toBeVisible();
  ```

## 2. Query like a User

### Follow React Testing Library query priority

Prefer queries accessible to everyone. `getByRole` is the default. `getByTestId` is a last resort because Users cannot see or hear test identifiers.

Never:
  ```javascript
  screen.getByTestId('submit-button')
  container.querySelector('.btn-primary')
  ```

Example:
  ```javascript
  screen.getByRole('button', { name: /submit/i })
  screen.getByLabelText(/email address/i)
  screen.getByText(/no results found/i)
  ```

### Choose the right query variant

`getBy` throws on missing elements and asserts presence. `queryBy` returns null and asserts absence. `findBy` returns a Promise for async elements.

Example:
  ```javascript
  expect(screen.getByRole('alert')).toBeInTheDocument();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  const alert = await screen.findByRole('alert');
  expect(alert).toHaveTextContent(/saved/i);
  ```

## 3. Simulate real interaction sequences

### Use userEvent over fireEvent

`userEvent` fires complete interaction sequences. `fireEvent` dispatches a single synthetic event that misses real browser behavior.

Never:
  ```javascript
  fireEvent.change(input, { target: { value: 'hello' } });
  ```

Example:
  ```javascript
  const user = userEvent.setup();
  await user.type(screen.getByLabelText(/email/i), 'test@example.com');
  await user.click(screen.getByRole('button', { name: /submit/i }));
  ```

## 4. Mock at the network edge

### Use Mock Service Worker for APIs

Mock Service Worker intercepts at the network level and works with any HTTP client. The same mocks serve unit, integration, and end-to-end tests.

Example:
  ```javascript
  import { http, HttpResponse } from 'msw';
  export const handlers = [
    http.get('/api/users', () => {
      return HttpResponse.json([{ id: 1, name: 'Alice' }]);
    }),
  ];

  import { setupServer } from 'msw/node';
  import { handlers } from './mocks/handlers';
  export const server = setupServer(...handlers);
  beforeAll(() => server.listen());
  afterEach(() => server.resetHandlers());
  afterAll(() => server.close());

  server.use(
    http.get('/api/users', () => new HttpResponse(null, { status: 500 }))
  );
  ```

## 5. Prefer integration tests

### Render real component trees

The Testing Trophy says integration tests provide the best confidence-per-time return. Render real component trees, mock only the network edge, and simulate real Critical Paths. Never test internal state, instance methods, lifecycle calls, or CSS class names.

Example:
  ```javascript
  const user = userEvent.setup();
  render(<TodoApp />);
  await user.type(screen.getByLabelText(/new todo/i), 'Buy groceries');
  await user.click(screen.getByRole('button', { name: /add/i }));
  expect(await screen.findByText('Buy groceries')).toBeInTheDocument();
  ```

## 6. Wait only where the User waits

### Use findBy for async elements and waitFor for complex assertions

`findBy` combines `getBy` and `waitFor` and is preferred for single-element async queries. Use `waitFor` only for complex conditions. Never put side effects inside `waitFor`.

Never:
  ```javascript
  const btn = await waitFor(() => screen.getByRole('button'));
  await waitFor(() => {
    fireEvent.click(button);
    expect(result).toBeInTheDocument();
  });
  ```

Example:
  ```javascript
  const btn = await screen.findByRole('button');
  await user.click(button);
  await waitFor(() => expect(result).toBeInTheDocument());
  ```

## 7. Cover common User Interface states

### Test forms, modals, and error states

Example:
  ```javascript
  const handleSubmit = vi.fn();
  const user = userEvent.setup();
  render(<Form onSubmit={handleSubmit} />);
  await user.type(screen.getByLabelText(/name/i), 'Alice');
  await user.click(screen.getByRole('button', { name: /submit/i }));
  expect(handleSubmit).toHaveBeenCalledWith({ name: 'Alice' });

  await user.click(screen.getByRole('button', { name: /open/i }));
  expect(screen.getByRole('dialog')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: /close/i }));
  await waitForElementToBeRemoved(() => screen.queryByRole('dialog'));
  ```

### Use renderHook for shared hooks only

If a hook is used by one component, test it through that component. Use `renderHook` only for shared hooks.

Example:
  ```javascript
  import { renderHook, act } from '@testing-library/react';

  test('useCounter increments', () => {
    const { result } = renderHook(() => useCounter());
    expect(result.current.count).toBe(0);
    act(() => result.current.increment());
    expect(result.current.count).toBe(1);
  });
  ```

## 8. Add accessibility checks

### Use axe with jsdom

Use `vitest-axe` or `jest-axe` to catch Web Content Accessibility Guidelines violations automatically. Use the `jsdom` environment, not `happy-dom`, because of axe compatibility issues.

Example:
  ```javascript
  import { axe } from 'vitest-axe';

  test('form has no accessibility violations', async () => {
    const { container } = render(<LoginForm />);
    expect(await axe(container)).toHaveNoViolations();
  });
  ```

### Configure Vitest for React testing

Set `environment: 'jsdom'`, `clearMocks: true`, and `restoreMocks: true`. The setup file imports `@testing-library/jest-dom/vitest` and `vitest-axe/extend-expect`.

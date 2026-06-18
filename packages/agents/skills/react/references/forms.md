# React 19 Form Patterns

## Use useActionState for Form Submissions

useActionState manages pending state, error handling, and form submission in one hook. The action function receives previous state and FormData, returns new state.

```tsx
// Incorrect: Manual state management for forms
function CreatePost() {
  const [error, setError] = useState(null);
  const [isPending, setIsPending] = useState(false);

  async function handleSubmit(e) {
    e.preventDefault();
    setIsPending(true);
    const result = await createPost(new FormData(e.target));
    if (result.error) setError(result.error);
    setIsPending(false);
  }

  return <form onSubmit={handleSubmit}>...</form>;
}

// Correct: useActionState handles pending, errors, and submission
function CreatePost() {
  const [error, submitAction, isPending] = useActionState(
    async (previousState, formData) => {
      const result = await createPost(formData.get("title"));
      if (result.error) return result.error;
      redirect("/posts");
      return null;
    },
    null,
  );

  return (
    <form action={submitAction}>
      <input type="text" name="title" />
      <button type="submit" disabled={isPending}>Create</button>
      {error && <p>{error}</p>}
    </form>
  );
}
```

## Return Validation Errors as State, Throw Unexpected Errors

Known validation failures display inline. Unexpected errors propagate to the nearest Error Boundary.

```tsx
// Incorrect: Catching everything and returning as state
const [state, action] = useActionState(async (prev, formData) => {
  try {
    await saveRecord(formData);
    return null;
  } catch (e) {
    return e.message; // Hides unexpected errors from Error Boundary
  }
}, null);

// Correct: Return known errors, throw unexpected ones
const [state, action] = useActionState(async (prev, formData) => {
  const result = await saveRecord(formData);
  if (result.validationError) {
    return { error: result.validationError }; // Inline display
  }
  // Unexpected errors (network, server 500) throw naturally
  // and are caught by the nearest ErrorBoundary
  return null;
}, null);
```

## Use useFormStatus for Submit Button Pending State

useFormStatus reads the parent form's pending state without prop drilling. The component must be rendered inside a `<form>`.

```tsx
// Incorrect: Passing isPending as a prop through component tree
function Form({ isPending }) {
  return <SubmitButton isPending={isPending} />;
}

// Correct: useFormStatus reads parent form state automatically
function SubmitButton() {
  const { pending } = useFormStatus();
  return (
    <button type="submit" disabled={pending}>
      {pending ? "Submitting..." : "Submit"}
    </button>
  );
}

function Form() {
  return (
    <form action={submitAction}>
      <input name="email" />
      <SubmitButton />
    </form>
  );
}
```

## Use useOptimistic Inside startTransition

useOptimistic provides immediate UI feedback while async operations complete. The setter must be called inside startTransition or a form action.

```tsx
// Incorrect: Calling setOptimistic outside of a transition
const [optimisticTodos, addOptimistic] = useOptimistic(todos, reducer);
addOptimistic(newTodo); // Warning: not inside a transition

// Correct: Wrap in startTransition with async action
function TodoList({ todos, addTodoAction }) {
  const [optimisticTodos, addOptimistic] = useOptimistic(
    todos,
    (current, newTodo) => [...current, { ...newTodo, pending: true }]
  );

  function handleAdd(text) {
    const newTodo = { id: crypto.randomUUID(), text };
    startTransition(async () => {
      addOptimistic(newTodo);
      await addTodoAction(newTodo);
    });
  }

  return (
    <ul>
      {optimisticTodos.map(todo => (
        <li key={todo.id}>
          {todo.text} {todo.pending && "(Saving...)"}
        </li>
      ))}
    </ul>
  );
}
```

## Prefer Uncontrolled Forms with React 19

React 19 forms auto-reset on successful submission. Use uncontrolled inputs (no onChange/value) unless you need real-time validation.

```tsx
// Unnecessary: Controlled inputs for simple submission
function SearchForm() {
  const [query, setQuery] = useState("");
  return (
    <form action={search}>
      <input value={query} onChange={e => setQuery(e.target.value)} name="q" />
    </form>
  );
}

// Better: Uncontrolled — React reads FormData, resets on success
function SearchForm() {
  return (
    <form action={search}>
      <input name="q" defaultValue="" />
      <button type="submit">Search</button>
    </form>
  );
}

// Controlled is correct when you need real-time validation
function EmailForm() {
  const [email, setEmail] = useState("");
  const isValid = email.includes("@");
  return (
    <form action={submitAction}>
      <input value={email} onChange={e => setEmail(e.target.value)} name="email" />
      {email && !isValid && <p>Invalid email</p>}
    </form>
  );
}
```

## Enable Progressive Enhancement with Permalink

The third argument to useActionState enables form submission before JavaScript loads. The form works as a traditional HTTP form until hydration completes.

```tsx
// Correct: Forms work before JS loads
function UpdateName() {
  const [state, submitAction] = useActionState(
    updateNameAction,
    null,
    "/name/update" // Permalink — fallback URL before JS loads
  );

  return (
    <form action={submitAction}>
      <input name="name" />
      <button type="submit">Update</button>
      {state?.error && <p>{state.error}</p>}
    </form>
  );
}
```

## Use requestFormReset for Manual Form Reset

When you need to reset a form after a fetcher submission or when using controlled inputs, use requestFormReset.

```tsx
// Correct: Reset form after optimistic update
function MessageForm({ sendMessage }) {
  const formRef = useRef();

  async function formAction(formData) {
    addOptimisticMessage(formData.get("message"));
    formRef.current.reset(); // Or requestFormReset(formRef.current)
    await sendMessage(formData);
  }

  return (
    <form action={formAction} ref={formRef}>
      <input type="text" name="message" />
      <button type="submit">Send</button>
    </form>
  );
}
```

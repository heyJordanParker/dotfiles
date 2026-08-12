# Forms

One Process: let React 19 form primitives own submission state, return expected validation errors as state, and verify pending, optimistic, reset, and no-JavaScript behavior.

## 1. Use useActionState for submissions

### Let useActionState own pending and error state

`useActionState` manages pending state, error handling, and form submission in one hook. The action receives previous state and `FormData`, then returns new state.

Never:
  ```tsx
  function CreatePost() {
    const [error, setError] = useState(null);
    const [isPending, setIsPending] = useState(false);

    async function handleSubmit(event) {
      event.preventDefault();
      setIsPending(true);
      const result = await createPost(new FormData(event.target));
      if (result.error) setError(result.error);
      setIsPending(false);
    }

    return <form onSubmit={handleSubmit}>...</form>;
  }
  ```

Example:
  ```tsx
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

## 2. Separate expected and unexpected failures

### Return validation errors and throw unexpected errors

Known validation failures display inline. Unexpected errors propagate to the nearest Error Boundary.

Never:
  ```tsx
  const [state, action] = useActionState(async (prev, formData) => {
    try {
      await saveRecord(formData);
      return null;
    } catch (error) {
      return error.message;
    }
  }, null);
  ```

Example:
  ```tsx
  const [state, action] = useActionState(async (prev, formData) => {
    const result = await saveRecord(formData);
    if (result.validationError) {
      return { error: result.validationError };
    }
    return null;
  }, null);
  ```

## 3. Read form pending state inside the form

### Use useFormStatus for submit buttons

`useFormStatus` reads the parent form's pending state without prop drilling. The component must render inside a `<form>`.

Never:
  ```tsx
  function Form({ isPending }) {
    return <SubmitButton isPending={isPending} />;
  }
  ```

Example:
  ```tsx
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

## 4. Wrap optimistic updates in a transition

### Use useOptimistic inside startTransition or a form action

`useOptimistic` provides immediate User Interface feedback while async operations complete. Its setter must be called inside `startTransition` or a form action.

Never:
  ```tsx
  const [optimisticTodos, addOptimistic] = useOptimistic(todos, reducer);
  addOptimistic(newTodo);
  ```

Example:
  ```tsx
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

## 5. Prefer uncontrolled fields unless live validation is needed

### Use uncontrolled forms for simple React 19 submissions

React 19 forms auto-reset after successful submission. Use uncontrolled inputs unless real-time validation is required.

Never:
  ```tsx
  function SearchForm() {
    const [query, setQuery] = useState("");
    return (
      <form action={search}>
        <input value={query} onChange={event => setQuery(event.target.value)} name="q" />
      </form>
    );
  }
  ```

Example:
  ```tsx
  function SearchForm() {
    return (
      <form action={search}>
        <input name="q" defaultValue="" />
        <button type="submit">Search</button>
      </form>
    );
  }
  ```

Example:
  ```tsx
  function EmailForm() {
    const [email, setEmail] = useState("");
    const isValid = email.includes("@");
    return (
      <form action={submitAction}>
        <input value={email} onChange={event => setEmail(event.target.value)} name="email" />
        {email && !isValid && <p>Invalid email</p>}
      </form>
    );
  }
  ```

## 6. Preserve behavior before JavaScript loads

### Use the permalink argument for progressive enhancement

The third argument to `useActionState` lets the form submit before JavaScript loads. The form works as a traditional HTTP form until hydration completes.

Example:
  ```tsx
  function UpdateName() {
    const [state, submitAction] = useActionState(
      updateNameAction,
      null,
      "/name/update"
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

## 7. Reset manually only when automatic reset does not apply

### Use requestFormReset for manual reset

Use `requestFormReset` after fetcher submissions or controlled-input flows where React's automatic reset does not apply.

Example:
  ```tsx
  function MessageForm({ sendMessage }) {
    const formRef = useRef();

    async function formAction(formData) {
      addOptimisticMessage(formData.get("message"));
      formRef.current.reset();
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

# Accessibility Best Practices

## Prefer Semantic HTML Over ARIA

Native elements carry built-in semantics and keyboard behavior. Only use ARIA when no native element exists.

**Incorrect:**
```jsx
<div role="button" tabIndex={0} onClick={handleClick}>Save</div>
<div role="navigation"><div role="list"><div role="listitem">Home</div></div></div>
```

**Correct:**
```jsx
<button onClick={handleClick}>Save</button>
<nav><ul><li><a href="/">Home</a></li></ul></nav>
```

Use `<main>`, `<header>`, `<footer>`, `<section>`, `<article>` for landmarks. Use `<fieldset>`/`<legend>` for form groups. Use `<dialog>` for modals.

## Use useId() for Unique Accessibility IDs

Hardcoded IDs break when a component renders multiple times. `useId()` generates unique, stable IDs.

**Incorrect:**
```jsx
<label htmlFor="email">Email</label>
<input id="email" type="email" />
```

**Correct:**
```jsx
function EmailField() {
  const id = useId();
  const errorId = useId();
  return (
    <>
      <label htmlFor={id}>Email</label>
      <input id={id} type="email" aria-describedby={error ? errorId : undefined} />
      {error && <p id={errorId} role="alert">{error}</p>}
    </>
  );
}
```

## Provide Keyboard Equivalents for Every Mouse Interaction

Every mouse-accessible feature must work via keyboard. Only use `tabIndex` values of `0` or `-1` — never positive.

Incorrect — only mouse users can dismiss:
```jsx
<div onClick={handleOutsideClick}><Popover /></div>
```

Correct — blur/focus handles keyboard:
```jsx
<div onBlur={handleBlur} onFocus={handleFocus}>
  <button aria-haspopup="true" aria-expanded={isOpen} onClick={toggle}>Menu</button>
  {isOpen && <ul role="menu">{children}</ul>}
</div>
```

## Manage Focus in Modals: Save, Move, Trap, Restore

When a modal opens: save the trigger element, move focus into the modal, trap Tab/Shift+Tab, close on Escape, restore focus on close. Set `aria-hidden="true"` or `inert` on background content.

```jsx
function Modal({ isOpen, onClose, title, children }) {
  const modalRef = useRef(null);
  const previousFocusRef = useRef(null);
  const titleId = useId();

  useEffect(() => {
    if (isOpen) {
      previousFocusRef.current = document.activeElement;
      modalRef.current?.focus();
    }
    return () => { previousFocusRef.current?.focus(); };
  }, [isOpen]);

  if (!isOpen) return null;
  return createPortal(
    <div ref={modalRef} role="dialog" aria-modal="true"
      aria-labelledby={titleId} tabIndex={-1}
      onKeyDown={e => { if (e.key === 'Escape') onClose(); }}>
      <h2 id={titleId}>{title}</h2>
      {children}
    </div>, document.body);
}
```

Use `focus-trap-react` to simplify trapping. Better yet, use the native `<dialog>` element.

## Prefer the Native dialog Element

`<dialog>` with `showModal()` provides built-in focus trapping, Escape to close, backdrop, and background `inert` — no JavaScript focus management needed.

```jsx
function NativeDialog({ isOpen, onClose, children }) {
  const ref = useRef(null);
  useEffect(() => {
    if (isOpen) ref.current?.showModal();
    else ref.current?.close();
  }, [isOpen]);
  return <dialog ref={ref} onClose={onClose}>{children}</dialog>;
}
```

## Mount Live Regions Before Content Changes

Screen readers only announce changes to existing live regions. Conditionally render content inside the container, not the container itself.

**Incorrect:**
```jsx
{error && <div role="alert">{error}</div>}
```

**Correct:**
```jsx
<div role="alert" aria-live="assertive">{error}</div>
```

Use `aria-live="polite"` for status updates. Use `aria-live="assertive"` or `role="alert"` for errors.

## Build Accessible Forms

Every input needs a label via `htmlFor`/`id` or wrapping. Link errors with `aria-describedby`. Group related controls with `<fieldset>` and `<legend>`.

```jsx
function TextField({ label, error, required }) {
  const inputId = useId();
  const errorId = useId();
  return (
    <div>
      <label htmlFor={inputId}>{label}{required && <span aria-hidden="true"> *</span>}</label>
      <input id={inputId} aria-required={required} aria-invalid={!!error}
        aria-describedby={error ? errorId : undefined} />
      {error && <p id={errorId} role="alert">{error}</p>}
    </div>
  );
}
```

```jsx
<fieldset>
  <legend>Choose a size</legend>
  <label><input type="radio" name="size" value="s" /> Small</label>
  <label><input type="radio" name="size" value="m" /> Medium</label>
</fieldset>
```

## Respect prefers-reduced-motion

Default to no animation. Opt-in when the user has no motion preference.

```css
.animated { transition: none; }
@media (prefers-reduced-motion: no-preference) {
  .animated { transition: transform 300ms ease; }
}
```

For JS animations, use a hook:
```jsx
function usePrefersReducedMotion() {
  const [reduced, setReduced] = useState(true);
  useEffect(() => {
    const mql = window.matchMedia('(prefers-reduced-motion: no-preference)');
    setReduced(!mql.matches);
    const listener = (e) => setReduced(!e.matches);
    mql.addEventListener('change', listener);
    return () => mql.removeEventListener('change', listener);
  }, []);
  return reduced;
}
```

## Enable eslint-plugin-jsx-a11y

Use `plugin:jsx-a11y/recommended` to catch missing alt text, click handlers without key events, positive tabIndex, missing labels, invalid ARIA, and redundant roles at build time.

```json
{ "extends": ["plugin:jsx-a11y/recommended"], "plugins": ["jsx-a11y"] }
```

## Add Skip Links and Route Announcements for SPAs

Skip links let keyboard users bypass navigation. Route announcers notify screen readers of client-side page changes.

```jsx
<a href="#main-content" className="skip-link">Skip to main content</a>
<main id="main-content" tabIndex={-1}>{/* content */}</main>
```

```jsx
function RouteAnnouncer() {
  const [message, setMessage] = useState('');
  const location = useLocation();
  useEffect(() => {
    if (document.title) setMessage(`Navigated to ${document.title}`);
  }, [location.pathname]);
  return (
    <div role="status" aria-live="assertive" aria-atomic="true"
      style={{ position: 'absolute', width: '1px', height: '1px',
        overflow: 'hidden', clip: 'rect(0,0,0,0)' }}>{message}</div>
  );
}
```

## Use Visually Hidden Content for Screen Reader Context

Use the visually-hidden pattern for text equivalents of icons — not `display: none` which hides from screen readers too.

```jsx
const visuallyHidden = {
  position: 'absolute', width: '1px', height: '1px', padding: 0,
  margin: '-1px', overflow: 'hidden', clip: 'rect(0,0,0,0)',
  whiteSpace: 'nowrap', border: 0,
};
<button><TrashIcon aria-hidden="true" /><span style={visuallyHidden}>Delete</span></button>
```

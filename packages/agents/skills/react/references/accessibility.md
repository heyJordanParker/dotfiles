# Accessibility

One Process: use native semantics first, make every interaction keyboard-accessible, preserve focus, and verify assistive output.

## 1. Start with native elements

### Prefer semantic HTML over ARIA

Native elements carry built-in semantics and keyboard behavior. Use ARIA only when no native element exists.

Never:
  ```jsx
  <div role="button" tabIndex={0} onClick={handleClick}>Save</div>
  <div role="navigation"><div role="list"><div role="listitem">Home</div></div></div>
  ```

Example:
  ```jsx
  <button onClick={handleClick}>Save</button>
  <nav><ul><li><a href="/">Home</a></li></ul></nav>
  ```

Use `<main>`, `<header>`, `<footer>`, `<section>`, and `<article>` for landmarks. Use `<fieldset>` and `<legend>` for form groups. Use `<dialog>` for modals.

## 2. Give every relationship a stable identifier

### Use useId for accessibility identifiers

Hardcoded identifiers break when a component renders multiple times. `useId()` generates unique, stable identifiers.

Never:
  ```jsx
  <label htmlFor="email">Email</label>
  <input id="email" type="email" />
  ```

Example:
  ```jsx
  function EmailField({ error }) {
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

## 3. Make every interaction keyboard-accessible

### Provide keyboard equivalents for mouse interactions

Every mouse-accessible feature must work by keyboard. Use only `tabIndex={0}` or `tabIndex={-1}`; never positive values.

Never:
  ```jsx
  <div onClick={handleOutsideClick}><Popover /></div>
  ```

Example:
  ```jsx
  <div onBlur={handleBlur} onFocus={handleFocus}>
    <button aria-haspopup="true" aria-expanded={isOpen} onClick={toggle}>Menu</button>
    {isOpen && <ul role="menu">{children}</ul>}
  </div>
  ```

## 4. Manage focus for modals

### Save, move, trap, and restore focus

When a modal opens: save the trigger element, move focus into the modal, trap Tab and Shift+Tab, close on Escape, restore focus on close, and set `aria-hidden="true"` or `inert` on background content.

Example:
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

### Prefer the native dialog element

`<dialog>` with `showModal()` provides built-in focus trapping, Escape close, backdrop, and background `inert`.

Example:
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

Use `focus-trap-react` only when native `<dialog>` does not fit.

## 5. Announce changes after the announcer exists

### Mount live regions before content changes

Screen readers announce changes to existing live regions. Conditionally render content inside the container, not the container itself.

Never:
  ```jsx
  {error && <div role="alert">{error}</div>}
  ```

Example:
  ```jsx
  <div role="alert" aria-live="assertive">{error}</div>
  ```

Use `aria-live="polite"` for status updates. Use `aria-live="assertive"` or `role="alert"` for errors.

## 6. Build accessible forms

### Label every input and link errors

Every input needs a label through `htmlFor` and `id` or wrapping. Link errors with `aria-describedby`. Group related controls with `<fieldset>` and `<legend>`.

Example:
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

Example:
  ```jsx
  <fieldset>
    <legend>Choose a size</legend>
    <label><input type="radio" name="size" value="s" /> Small</label>
    <label><input type="radio" name="size" value="m" /> Medium</label>
  </fieldset>
  ```

## 7. Respect User motion preference

### Default to no animation

Opt in only when the User has no motion preference.

Example:
  ```css
  .animated { transition: none; }
  @media (prefers-reduced-motion: no-preference) {
    .animated { transition: transform 300ms ease; }
  }
  ```

Example:
  ```jsx
  function usePrefersReducedMotion() {
    const [reduced, setReduced] = useState(true);
    useEffect(() => {
      const mql = window.matchMedia('(prefers-reduced-motion: no-preference)');
      setReduced(!mql.matches);
      const listener = (event) => setReduced(!event.matches);
      mql.addEventListener('change', listener);
      return () => mql.removeEventListener('change', listener);
    }, []);
    return reduced;
  }
  ```

## 8. Add static checks and route context

### Enable eslint-plugin-jsx-a11y

Use `plugin:jsx-a11y/recommended` to catch missing alternative text, click handlers without key events, positive `tabIndex`, missing labels, invalid ARIA, and redundant roles at build time.

Example:
  ```json
  { "extends": ["plugin:jsx-a11y/recommended"], "plugins": ["jsx-a11y"] }
  ```

### Add skip links and route announcements for single-page applications

Skip links let keyboard Users bypass navigation. Route announcers notify screen readers of client-side page changes.

Example:
  ```jsx
  <a href="#main-content" className="skip-link">Skip to main content</a>
  <main id="main-content" tabIndex={-1}>{/* content */}</main>
  ```

Example:
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

### Use visually hidden content for screen reader context

Use visually hidden text for icon equivalents, not `display: none`, which hides content from screen readers.

Example:
  ```jsx
  const visuallyHidden = {
    position: 'absolute', width: '1px', height: '1px', padding: 0,
    margin: '-1px', overflow: 'hidden', clip: 'rect(0,0,0,0)',
    whiteSpace: 'nowrap', border: 0,
  };
  <button><TrashIcon aria-hidden="true" /><span style={visuallyHidden}>Delete</span></button>
  ```

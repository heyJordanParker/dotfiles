# Effects

One Process: use an Effect only to synchronize with an external system, and replace every other Effect with render logic, event handlers, keys, state ownership, or purpose-built hooks.

## 1. Confirm there is an external system

### Effects are escape hatches

Use Effects for network, DOM, and third-party widget synchronization. If no external system is involved, do not use an Effect.

## 2. Derive values during render

### Derive state during render, not in Effects

If a value can be calculated from props or state, compute it inline. Effects cause unnecessary render cycles.

Never:
  ```jsx
  const [fullName, setFullName] = useState('');
  useEffect(() => { setFullName(firstName + ' ' + lastName); }, [firstName, lastName]);
  ```

Example:
  ```jsx
  const fullName = firstName + ' ' + lastName;
  ```

### Cache expensive calculations with useMemo, not Effects

Use `useMemo` for computations that are slow, measured around one millisecond or more. Measure with `console.time()` before assuming cost.

Never:
  ```jsx
  const [visibleTodos, setVisibleTodos] = useState([]);
  useEffect(() => { setVisibleTodos(getFilteredTodos(todos, filter)); }, [todos, filter]);
  ```

Example:
  ```jsx
  const visibleTodos = useMemo(() => getFilteredTodos(todos, filter), [todos, filter]);
  ```

## 3. Reset or adjust state through ownership

### Reset state with key, not Effects

Pass the prop as a `key` to destroy and recreate the component with fresh state.

Never:
  ```jsx
  useEffect(() => { setComment(''); }, [userId]);
  ```

Example:
  ```jsx
  <Profile userId={userId} key={userId} />
  ```

### Adjust partial state by restructuring, not Effects

Store identifiers instead of derived objects so state does not need adjusting when props change.

Never:
  ```jsx
  const [selection, setSelection] = useState(null);
  useEffect(() => { setSelection(null); }, [items]);
  ```

Example:
  ```jsx
  const [selectedId, setSelectedId] = useState(null);
  const selection = items.find(item => item.id === selectedId) ?? null;
  ```

## 4. Keep event logic in event handlers

### Put event-specific logic in event handlers

Ask whether the behavior was caused by a User interaction or by the component appearing on screen. Interactions belong in event handlers. Display-triggered analytics belong in Effects.

Never:
  ```jsx
  useEffect(() => {
    if (product.isInCart) showNotification(`Added ${product.name} to cart!`);
  }, [product]);
  ```

Example:
  ```jsx
  function handleBuyClick() {
    addToCart(product);
    showNotification(`Added ${product.name} to cart!`);
  }
  ```

Form posts caused by clicks belong in event handlers. Share logic between handlers by extracting a plain function, not an Effect.

### Avoid Effect chains

Multiple Effects triggering each other through `setState` cause cascading re-renders. Derive what you can during render and compute the rest in the event handler.

Never:
  ```jsx
  useEffect(() => { if (card?.gold) setGoldCardCount(count => count + 1); }, [card]);
  useEffect(() => { if (goldCardCount > 3) { setRound(round => round + 1); setGoldCardCount(0); } }, [goldCardCount]);
  ```

Example:
  ```jsx
  const isGameOver = round > 5;

  function handlePlaceCard(nextCard) {
    setCard(nextCard);
    if (nextCard.gold) {
      if (goldCardCount < 3) setGoldCardCount(goldCardCount + 1);
      else { setGoldCardCount(0); setRound(round + 1); }
    }
  }
  ```

## 5. Move initialization and parent communication out of Effects

### Run application initialization at module level

Strict Mode double-invokes Effects. For one-time initialization, use module-level execution.

Example:
  ```jsx
  if (typeof window !== 'undefined') {
    checkAuthToken();
    loadDataFromLocalStorage();
  }
  ```

### Notify parents in the same handler or lift state

Do not use Effects to call parent callbacks after state changes; that causes two render passes.

Never:
  ```jsx
  useEffect(() => { onChange(isOn); }, [isOn, onChange]);
  ```

Example:
  ```jsx
  function updateToggle(nextIsOn) { setIsOn(nextIsOn); onChange(nextIsOn); }
  ```

Example:
  ```jsx
  function Toggle({ isOn, onChange }) {
    return <button onClick={() => onChange(!isOn)} />;
  }
  ```

### Let parents fetch and pass data down

Do not fetch in a child and pass results up through Effect callbacks. Data moves down.

Never:
  ```jsx
  function Child({ onFetched }) {
    const data = useSomeAPI();
    useEffect(() => { if (data) onFetched(data); }, [onFetched, data]);
  }
  ```

Example:
  ```jsx
  function Parent() {
    const data = useSomeAPI();
    return <Child data={data} />;
  }
  ```

## 6. Use dedicated APIs for subscriptions and async safety

### Use useSyncExternalStore for external subscriptions

Do not manually subscribe to browser APIs with Effects. `useSyncExternalStore` handles server rendering.

Never:
  ```jsx
  const [isOnline, setIsOnline] = useState(true);
  useEffect(() => {
    const update = () => setIsOnline(navigator.onLine);
    window.addEventListener('online', update);
    window.addEventListener('offline', update);
    return () => { /* cleanup */ };
  }, []);
  ```

Example:
  ```jsx
  const isOnline = useSyncExternalStore(subscribe, () => navigator.onLine, () => true);
  ```

### Add cleanup to data-fetching Effects

Without cleanup, rapid state changes cause race conditions where stale responses overwrite fresh ones.

Example:
  ```jsx
  useEffect(() => {
    let ignore = false;
    fetchResults(query).then(json => { if (!ignore) setResults(json); });
    return () => { ignore = true; };
  }, [query]);
  ```

Prefer a custom hook, TanStack Query, or framework data fetching when available.

## 7. Make raw Effects visible in Review

### Ban raw useEffect with ESLint

Configure `no-restricted-syntax` to flag `useEffect` calls. Provide a `useMountEffect` escape hatch only for rare genuine Effects.

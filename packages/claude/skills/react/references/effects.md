# Effects Best Practices

Effects are an escape hatch for synchronizing with external systems (network, DOM, third-party widgets). If no external system is involved, you don't need an Effect.

## Derive State During Render, Not in Effects

If a value can be calculated from existing props or state, compute it inline. Effects cause unnecessary render cycles.

**Incorrect:**
```jsx
const [fullName, setFullName] = useState('');
useEffect(() => { setFullName(firstName + ' ' + lastName); }, [firstName, lastName]);
```

**Correct:**
```jsx
const fullName = firstName + ' ' + lastName;
```

## Cache Expensive Calculations with useMemo, Not Effects

Use `useMemo` for computations that are slow (~1ms+). Measure with `console.time()` before assuming something is expensive.

**Incorrect:**
```jsx
const [visibleTodos, setVisibleTodos] = useState([]);
useEffect(() => { setVisibleTodos(getFilteredTodos(todos, filter)); }, [todos, filter]);
```

**Correct:**
```jsx
const visibleTodos = useMemo(() => getFilteredTodos(todos, filter), [todos, filter]);
```

## Reset State with key, Not Effects

Pass the prop as a `key` to destroy and recreate the component with fresh state.

**Incorrect:**
```jsx
useEffect(() => { setComment(''); }, [userId]);
```

**Correct:**
```jsx
<Profile userId={userId} key={userId} />
```

## Adjust Partial State by Restructuring, Not Effects

Store IDs instead of derived objects so state doesn't need "adjusting" when props change.

**Incorrect:**
```jsx
const [selection, setSelection] = useState(null);
useEffect(() => { setSelection(null); }, [items]);
```

**Correct:**
```jsx
const [selectedId, setSelectedId] = useState(null);
const selection = items.find(item => item.id === selectedId) ?? null;
```

## Put Event-Specific Logic in Event Handlers

Ask: "Was this caused by a user interaction or by the component appearing on screen?" Interactions belong in event handlers. Only display-triggered logic (analytics) belongs in Effects.

**Incorrect:**
```jsx
useEffect(() => {
  if (product.isInCart) showNotification(`Added ${product.name} to cart!`);
}, [product]);
```

**Correct:**
```jsx
function handleBuyClick() {
  addToCart(product);
  showNotification(`Added ${product.name} to cart!`);
}
```

This applies to POST requests too. Analytics on display → Effect. Form submission on click → event handler. Share logic between handlers by extracting a plain function, not an Effect.

## Avoid Effect Chains — Compute in a Single Handler

Multiple Effects triggering each other via setState cause cascading re-renders. Derive what you can during render, compute the rest in the event handler.

**Incorrect:**
```jsx
useEffect(() => { if (card?.gold) setGoldCardCount(c => c + 1); }, [card]);
useEffect(() => { if (goldCardCount > 3) { setRound(r => r + 1); setGoldCardCount(0); } }, [goldCardCount]);
```

**Correct:**
```jsx
const isGameOver = round > 5; // derived during render

function handlePlaceCard(nextCard) {
  setCard(nextCard);
  if (nextCard.gold) {
    if (goldCardCount < 3) setGoldCardCount(goldCardCount + 1);
    else { setGoldCardCount(0); setRound(round + 1); }
  }
}
```

## Run App Initialization at Module Level

Strict Mode double-invokes Effects. For one-time init, use module-level execution.

```jsx
if (typeof window !== 'undefined') {
  checkAuthToken();
  loadDataFromLocalStorage();
}
```

## Notify Parents in the Same Handler or Lift State

Don't use Effects to call parent callbacks after state changes — causes two render passes.

**Incorrect:**
```jsx
useEffect(() => { onChange(isOn); }, [isOn, onChange]);
```

Correct — call in same handler (React batches both updates):
```jsx
function updateToggle(nextIsOn) { setIsOn(nextIsOn); onChange(nextIsOn); }
```

Correct — fully controlled:
```jsx
function Toggle({ isOn, onChange }) {
  return <button onClick={() => onChange(!isOn)} />;
}
```

## Let Parents Fetch and Pass Data Down

Don't fetch in a child and pass results up via Effect callbacks. Data flows down.

**Incorrect:**
```jsx
function Child({ onFetched }) {
  const data = useSomeAPI();
  useEffect(() => { if (data) onFetched(data); }, [onFetched, data]);
}
```

**Correct:**
```jsx
function Parent() {
  const data = useSomeAPI();
  return <Child data={data} />;
}
```

## Use useSyncExternalStore for External Subscriptions

Don't manually subscribe to browser APIs with Effects. `useSyncExternalStore` is purpose-built and handles SSR.

**Incorrect:**
```jsx
const [isOnline, setIsOnline] = useState(true);
useEffect(() => {
  const update = () => setIsOnline(navigator.onLine);
  window.addEventListener('online', update);
  window.addEventListener('offline', update);
  return () => { /* cleanup */ };
}, []);
```

**Correct:**
```jsx
const isOnline = useSyncExternalStore(subscribe, () => navigator.onLine, () => true);
```

## Always Add Cleanup to Data-Fetching Effects

Without cleanup, rapid state changes cause race conditions where stale responses overwrite fresh ones.

```jsx
useEffect(() => {
  let ignore = false;
  fetchResults(query).then(json => { if (!ignore) setResults(json); });
  return () => { ignore = true; };
}, [query]);
```

Extract into a custom Hook or use TanStack Query / framework built-in data fetching.

## Ban Raw useEffect with ESLint

Configure `no-restricted-syntax` to flag `useEffect` calls. Provide a `useMountEffect` escape hatch for the rare cases where an Effect is genuinely needed.

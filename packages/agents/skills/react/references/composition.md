# Component Composition Patterns

## Avoid Boolean Prop Proliferation

Each boolean prop doubles the number of possible states. N booleans create 2^N combinations, most of which are impossible. The type system allows all of them. Use composition instead.

**Incorrect:**

```tsx
function Composer({ isThread, isDMThread, isEditing, isForwarding, ...props }: Props) {
  return (
    <form>
      <Input />
      {isDMThread ? <AlsoSendToDMField /> : isThread ? <AlsoSendToChannelField /> : null}
      {isEditing ? <EditActions /> : isForwarding ? <ForwardActions /> : <DefaultActions />}
    </form>
  )
}
```

**Correct:**

```tsx
function ThreadComposer({ channelId }: { channelId: string }) {
  return (
    <Composer.Frame>
      <Composer.Input />
      <AlsoSendToChannelField id={channelId} />
      <Composer.Footer>
        <Composer.Formatting />
        <Composer.Submit />
      </Composer.Footer>
    </Composer.Frame>
  )
}

function EditComposer() {
  return (
    <Composer.Frame>
      <Composer.Input />
      <Composer.Footer>
        <Composer.CancelEdit />
        <Composer.SaveEdit />
      </Composer.Footer>
    </Composer.Frame>
  )
}
```

## Compound Components with Shared Context

Structure complex components as compound components. Each subcomponent accesses shared state via context, not props. Export as a namespace object.

```tsx
const ComposerContext = createContext<ComposerContextValue | null>(null)

function ComposerProvider({ children, state, actions, meta }: ProviderProps) {
  return <ComposerContext value={{ state, actions, meta }}>{children}</ComposerContext>
}

function ComposerInput() {
  const { state, actions: { update }, meta: { inputRef } } = use(ComposerContext)
  return <TextInput ref={inputRef} value={state.input}
    onChangeText={(text) => update((s) => ({ ...s, input: text }))} />
}

const Composer = { Provider: ComposerProvider, Frame: ComposerFrame, Input: ComposerInput, Submit: ComposerSubmit }
```

## Decouple State from UI via Generic Context Interface

Define a context interface with three parts: `state`, `actions`, `meta`. The provider is the only place that knows how state is managed. UI components consume the interface — they don't know if state comes from useState, Zustand, or a server sync.

```tsx
interface ComposerContextValue {
  state: { input: string; attachments: Attachment[]; isSubmitting: boolean }
  actions: { update: (fn: (s: State) => State) => void; submit: () => void }
  meta: { inputRef: React.RefObject<TextInput> }
}
```

Different providers implement the same interface. Swap the provider, keep the UI:

```tsx
function ChannelProvider({ channelId, children }: Props) {
  const { state, update, submit } = useGlobalChannel(channelId)
  return <Composer.Provider state={state} actions={{ update, submit }}>{children}</Composer.Provider>
}

function ForwardMessageProvider({ children }: Props) {
  const [state, setState] = useState(initialState)
  return <Composer.Provider state={state} actions={{ update: setState, submit: useForwardMessage() }}>{children}</Composer.Provider>
}
```

## Lift State into Provider Components

Move state into dedicated providers so sibling components outside the main UI can access it. The provider boundary defines data access, not the visual tree.

```tsx
function ForwardMessageDialog() {
  return (
    <ForwardMessageProvider>
      <Dialog>
        <Composer.Frame><Composer.Input /><Composer.Footer /></Composer.Frame>
        <MessagePreview />     {/* Outside Frame, reads state */}
        <ForwardButton />      {/* Outside Frame, calls submit */}
      </Dialog>
    </ForwardMessageProvider>
  )
}

function ForwardButton() {
  const { actions: { submit } } = use(ComposerContext)
  return <Button onPress={submit}>Forward</Button>
}
```

Don't trap state inside components, sync it up with useEffect, or read it from refs — lift it to a provider.

## Children Over Render Props

Use `children` for composing structure. Render props only when the parent provides data back.

```tsx
// Incorrect: render prop callbacks
<Composer renderHeader={() => <CustomHeader />} renderFooter={() => <Footer />} />

// Correct: children composition
<Composer.Frame>
  <CustomHeader />
  <Composer.Input />
  <Composer.Footer><Composer.Formatting /><Composer.Submit /></Composer.Footer>
</Composer.Frame>

// Exception: render prop when parent provides data
<List data={items} renderItem={({ item }) => <Item item={item} />} />
```

## React 19 API Changes

`ref` is a regular prop — no `forwardRef` wrapper needed. `use()` replaces `useContext()` and can be called conditionally.

**Incorrect (React 19):**

```tsx
const Input = forwardRef<HTMLInputElement, Props>((props, ref) => <input ref={ref} {...props} />)
const value = useContext(MyContext)
```

**Correct (React 19):**

```tsx
function Input({ ref, ...props }: Props & { ref?: React.Ref<HTMLInputElement> }) {
  return <input ref={ref} {...props} />
}
const value = use(MyContext)
```

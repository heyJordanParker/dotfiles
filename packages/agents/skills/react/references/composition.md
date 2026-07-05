# Composition

One Process: replace state combinations with composed pieces, put shared state behind a provider edge, and use React 19 component APIs.

## 1. Replace boolean mode props with composed components

### Avoid boolean prop proliferation

Each boolean prop doubles possible states. Multiple booleans create invalid combinations the type system still allows.

Never:
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

Example:
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

## 2. Build complex components as compound components

### Share state through context

Each subcomponent accesses shared state through context, not props. Export the parts as a namespace object.

Example:
  ```tsx
  const ComposerContext = createContext<ComposerContextValue | null>(null)

  function ComposerProvider({ children, state, actions, meta }: ProviderProps) {
    return <ComposerContext value={{ state, actions, meta }}>{children}</ComposerContext>
  }

  function ComposerInput() {
    const { state, actions: { update }, meta: { inputRef } } = use(ComposerContext)
    return <TextInput ref={inputRef} value={state.input}
      onChangeText={(text) => update((state) => ({ ...state, input: text }))} />
  }

  const Composer = { Provider: ComposerProvider, Frame: ComposerFrame, Input: ComposerInput, Submit: ComposerSubmit }
  ```

## 3. Separate state ownership from the User Interface

### Use a generic context contract

Define the context contract with `state`, `actions`, and `meta`. The provider owns how state is managed. User Interface components consume the contract and do not know whether state comes from `useState`, Zustand, or server sync.

Example:
  ```tsx
  interface ComposerContextValue {
    state: { input: string; attachments: Attachment[]; isSubmitting: boolean }
    actions: { update: (fn: (state: State) => State) => void; submit: () => void }
    meta: { inputRef: React.RefObject<TextInput> }
  }
  ```

Example:
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

## 4. Lift shared state into providers

### Put data access at the provider edge

Move state into dedicated providers so sibling components outside the main User Interface can access it.

Example:
  ```tsx
  function ForwardMessageDialog() {
    return (
      <ForwardMessageProvider>
        <Dialog>
          <Composer.Frame><Composer.Input /><Composer.Footer /></Composer.Frame>
          <MessagePreview />
          <ForwardButton />
        </Dialog>
      </ForwardMessageProvider>
    )
  }

  function ForwardButton() {
    const { actions: { submit } } = use(ComposerContext)
    return <Button onPress={submit}>Forward</Button>
  }
  ```

Never: trap state inside components, sync it upward with `useEffect`, or read shared state from refs.

## 5. Prefer children over render props

### Use render props only when the parent provides data back

Use `children` for ordinary composition. Render props are for a parent that calculates data and passes it to the child render function.

Never:
  ```tsx
  <Composer renderHeader={() => <CustomHeader />} renderFooter={() => <Footer />} />
  ```

Example:
  ```tsx
  <Composer.Frame>
    <CustomHeader />
    <Composer.Input />
    <Composer.Footer><Composer.Formatting /><Composer.Submit /></Composer.Footer>
  </Composer.Frame>
  ```

Example:
  ```tsx
  <List data={items} renderItem={({ item }) => <Item item={item} />} />
  ```

## 6. Use React 19 component APIs

### Treat ref as a regular prop

React 19 does not need a `forwardRef` wrapper.

Never:
  ```tsx
  const Input = forwardRef<HTMLInputElement, Props>((props, ref) => <input ref={ref} {...props} />)
  ```

Example:
  ```tsx
  function Input({ ref, ...props }: Props & { ref?: React.Ref<HTMLInputElement> }) {
    return <input ref={ref} {...props} />
  }
  ```

### Use use instead of useContext

`use()` replaces `useContext()` and can be called conditionally.

Never: `const value = useContext(MyContext)`.
Example: `const value = use(MyContext)`.

# Modeling classification

Use this Reference when a candidate item does not clearly qualify as a Place, UI Affordance, Code Affordance, Data Store, Wires Out, or Returns To.

## 1. Apply the Place test

### A Place blocks interaction with what is behind it
If the User cannot act on what is behind the area, create a Place. If the User can still act behind it, keep it as local state or a UI Affordance inside the current Place.
Example: modal, edit mode that transforms the whole screen, route page, backend edge.
Never: dropdown, tooltip, or checkbox-revealed fields as Places by default.

### Whole-screen modes are Places
Read and edit modes are separate Places when the available Affordances change across the whole screen.

### Subplaces group related Affordances inside a Place
Use `P2.1`, `P2.2`, and the same background in Mermaid when a subset belongs inside the parent Place.

### Place References detach crowded nested Places
Use an underscore name when a complex nested Place would clutter the parent. The Place Reference is a UI Affordance in the parent and wires to the full Place.
Example: `_letter-browser → P3`.

## 2. Classify User-facing items

### A UI Affordance is visible or actionable
Inputs, buttons, displays, scroll regions, spinners, and displayed text qualify.
Never: wrappers and layout containers.

### User-visible outputs are UI Affordances
Emails, notifications, and exported files are User-visible outputs and need Code Affordances wiring to them.

## 3. Classify code items

### A Code Affordance has identity in code
Methods, subscriptions, handlers, writes to stores, and framework mechanisms qualify when they connect behavior.
Example: `handleSubmit()`, `query$ subscription`, `detectChanges()`.
Never: internal transforms or navigation mechanisms that only explain how another Affordance happens.

### A navigation mechanism is not the destination
Wire directly to the Place reached by navigation.
Example: `openEditor() → P4`.
Never: `openEditor() → modalService.open() → P4`.

## 4. Classify state

### A Data Store is state that is written and read
Arrays, booleans, observables, Browser URL, localStorage, and Clipboard qualify when behavior reads them.
Never: configuration that is set once and never changes unless it changes behavior in the modeled path.

### Put the Data Store where the reader lives
A write from another Place reaches in. A shared Data Store belongs in a shared Data Stores section only when multiple Places read it.

## 5. Separate the relationships

### Containment is membership
The Place column says where an Affordance lives.
Example: `U3 ∈ P2.1`.

### Wires Out is control movement
Use Wires Out for triggers, calls, writes, and navigation.
Example: `U1 → N1`, `N1 → P2`.

### Returns To is data movement
Use Returns To for return values, display inputs, and Data Store reads.
Example: `N2 → U4`, `S1 → U5`.

## 6. Verify the classification

### Every display has an incoming data path
A UI Affordance that displays data needs a Returns To path from a Code Affordance or Data Store.

### Every Code Affordance connects
A handler has Wires Out, a query has Returns To, and a store write has a reader. A Code Affordance with neither is dead code or missing wiring.

### Every Data Store has a reader
A Data Store no behavior reads is unused or out of scope.

# Modeling Mermaid view

Mermaid is an optional visual view. The Affordance tables are the source of truth.

## 1. Start from the tables

### Never draw from memory
Every node and edge comes from the Affordance tables. If a diagram needs a node not in the tables, add it to the tables first or cut it from the diagram.

## 2. Draw Places as subgraphs

### The subgraph identifier matches the Place identifier
Use `P1`, `P2`, and `P2_1` as the subgraph identifiers so navigation wires can target the Place.
Example:
  ```mermaid
  flowchart TB
  subgraph P1["P1: CMS Page (Read Mode)"]
      U1["U1: Edit button"]
      N1["N1: toggleEditMode()"]
  end
  subgraph P2["P2: CMS Page (Edit Mode)"]
      U2["U2: Save button"]
  end
  N1 --> P2
  ```

### Label the system when the artifact crosses systems
Use `SYSTEM: Frontend`, `SYSTEM: Backend API`, or the project words for the systems involved.

## 3. Preserve Wires Out and Returns To

### Solid lines are Wires Out
Use `-->` for calls, triggers, writes, and navigation.
Example: `U1 --> N1`.

### Dashed lines are Returns To
Use `-.->` for return values, Data Store reads, and display inputs.
Example: `S1 -.-> U5`.

### Abbreviated movement needs a label
Use a labeled dashed line only when intermediate steps are outside this Modeling artifact.
Example: `S4 -.->|view query| U6`.

## 4. Use the established colors

Template:
  ```mermaid
  classDef ui fill:#ffb6c1,stroke:#d87093,color:#000
  classDef nonui fill:#d3d3d3,stroke:#808080,color:#000
  classDef store fill:#e6e6fa,stroke:#9370db,color:#000
  classDef chunk fill:#b3e5fc,stroke:#0288d1,color:#000,stroke-width:2px
  classDef placeRef fill:#ffb6c1,stroke:#d87093,stroke-width:2px,stroke-dasharray:5 5
  ```

## 5. Add path markers only when they aid reading

### Path markers explain a complex Critical Path
Use numbered green nodes connected with dashed lines when the path is not obvious from the edges.
Example:
  ```mermaid
  flowchart TB
      step1(["1 - CLICK EDIT"])
      step1 -.-> U1
      classDef step fill:#90EE90,stroke:#228B22,color:#000,font-weight:bold
      class step1 step
  ```
Never: `1. CLICK EDIT` or `1) CLICK EDIT`; Mermaid can parse those as lists.

## 6. Chunk crowded subsystems

### A Chunk has one wire in, one wire out, and many internals
Replace the subsystem in the main diagram with `name[["CHUNK: name"]]`, then show internals in a separate diagram with input and output edge markers.
Example:
  ```mermaid
  flowchart TB
      dynamicForm[["CHUNK: dynamic-form"]]
      N24 -->|formDefinition| dynamicForm
      dynamicForm -.->|valid$| U8
  ```

## 7. Verify against the tables

### Every table row appears once
Count Mermaid nodes against Affordance rows when the diagram is meant to be complete. Optional path markers and Chunk edge markers are the only extra nodes.

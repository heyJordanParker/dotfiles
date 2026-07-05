---
name: diagram
description: |
  Generate Excalidraw diagrams with a hand-drawn aesthetic. Pushes elements to a
  live Excalidraw canvas via HTTP API, or renders to PNG/SVG.
---

# Diagram Skill

- Diagrams are Excalidraw JSON with hand-drawn style.
- `drawbridge` is the command-line interface for live canvas work and rendering.
- Only specify fields that matter; the browser fills internal Excalidraw properties.

## 1. Plan the layout

### Group the drawing before emitting elements
Choose zones, flow direction, and element groups first. Prefer fewer larger elements over many small ones.
Never: start emitting boxes before deciding the layout direction.

### Use background zones for groups
Zones are low-opacity rectangles at opacity 30 to 35. Place zones first in the array because z-order is array order: first is back, last is front. Add a standalone text label inside the top-left corner.

## 2. Generate minimal elements

### Include only required fields and meaningful overrides
Every element needs `type`, unique `id`, `x`, and `y`. Skip defaults: strokeColor `#1e1e1e`, backgroundColor `transparent`, fillStyle `solid`, strokeWidth `2`, roughness `1`, and opacity `100`.
Never: dump full internal Excalidraw objects when the skeleton fields are enough.

### Use labeled shapes for boxes
Rectangle, ellipse, and diamond can take a `label` object. Text auto-centers. The container auto-resizes only when width and height are omitted. Include `roundness: { type: 3 }` for rounded corners.
Example: calculate explicit width with `max(120, text.length * fontSize * 0.6 + 40)`.

### Position standalone text from its left edge
Standalone text `x` is the left edge. To center at `cx`, use `x = cx - (text.length * fontSize * 0.5) / 2`. `textAlign` affects multi-line wrapping, not position.

### Bind arrows with skeleton fields
Arrows use `points` as `[dx,dy]` offsets from the arrow's own `x,y`. Bind arrows with `start` and `end`, and position the arrow's `x,y` at the source shape edge.
Never: `startBinding` or `endBinding`; those are internal Excalidraw fields.

### Use the palette and sizing rules
Use the palette in `element-format.md`. Body text is at least 16, titles at least 20, annotations at least 14, labeled shapes at least 60 high, and gaps 20 to 30px.
Never: font size below 14 or colors outside the palette.

## 3. Emit elements in drawing order

### Follow z-order and progressive flow
Array order is z-order. Emit elements progressively: zone, shape, its arrows, next shape, its arrows.
Example: `zone → box1 → arrow1 → box2 → arrow2 → box3`.
Never: `box1 → box2 → box3 → arrow1 → arrow2`.

## 4. Push or render with drawbridge

### Use `drawbridge` for live and file output
`drawbridge` auto-starts the server when needed.
Template:
  ```bash
  drawbridge push <session> '{"elements": [...]}'
  drawbridge append <session> '{"elements": [...]}'
  drawbridge clear <session>
  drawbridge undo <session>
  drawbridge get <session>
  drawbridge open <session>
  drawbridge render input.excalidraw output.png
  ```

### Wrap elements to save an `.excalidraw` file
Use the standard Excalidraw wrapper when saving to disk.
Template:
  ```json
  {
    "type": "excalidraw",
    "version": 2,
    "source": "https://excalidraw.com",
    "elements": [],
    "appState": { "viewBackgroundColor": "#ffffff", "gridSize": null },
    "files": {}
  }
  ```

## 5. Verify the diagram

### Check the drawing before reporting done
Confirm drawing order, font sizes, label widths, labeled-shape heights, arrow bindings, palette colors, and either live canvas push or rendered file output.
Never: report done before the diagram has been pushed to the live viewer or rendered to a file.

## References (each solves one problem)

- Element JSON, palette, and complete push and append Example → element-format.md

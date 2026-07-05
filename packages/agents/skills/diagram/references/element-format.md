# Diagram Element Format

- The browser fills internal Excalidraw properties automatically.
- Element skeletons should contain only fields that change behavior or appearance.

## 1. Start every element from the skeleton

### Include required fields and skip defaults
Every element needs `type`, unique `id`, `x`, and `y`. Skip defaults: strokeColor `#1e1e1e`, backgroundColor `transparent`, fillStyle `solid`, strokeWidth `2`, roughness `1`, and opacity `100`.

## 2. Use labeled shapes for primary nodes

### Put text in the `label` object
Labeled shapes work on rectangle, ellipse, and diamond. Text auto-centers. The container auto-resizes only when width and height are omitted. When dimensions are explicit, calculate width from the label.
Example:
  ```json
  { "type": "rectangle", "id": "r1", "x": 100, "y": 100, "width": 160, "height": 80,
    "roundness": { "type": 3 }, "backgroundColor": "#a5d8ff", "fillStyle": "solid",
    "label": { "text": "API Server", "fontSize": 20 } }
  ```
Example: `max(120, text.length * fontSize * 0.6 + 40)`.
Never: omit `roundness: { type: 3 }` on rounded boxes.

## 3. Position standalone text from its left edge

### Calculate centered text manually
Standalone text `x` is the left edge. `textAlign` affects multi-line wrapping, not position.
Example:
  ```json
  { "type": "text", "id": "t1", "x": 150, "y": 50, "text": "System Architecture", "fontSize": 28 }
  ```
Example: to center at `cx`, use `x = cx - (text.length * fontSize * 0.5) / 2`.

## 4. Draw arrows with offsets and skeleton bindings

### Use `points` as offsets from the arrow origin
Arrow `points` are `[dx, dy]` offsets from element `x,y`. `endArrowhead` can be `null`, `arrow`, `bar`, `dot`, or `triangle`. Arrow labels use the same `label` object shape.
Example:
  ```json
  { "type": "arrow", "id": "a1", "x": 300, "y": 150, "width": 200, "height": 0,
    "points": [[0,0],[200,0]], "endArrowhead": "arrow" }
  ```

### Bind arrows with `start` and `end`
Bind arrows to shapes so they stay connected. Position the arrow's `x,y` at the right edge of the source shape for left-to-right connections.
Example:
  ```json
  { "type": "arrow", "id": "a1", "x": 300, "y": 150, "width": 200, "height": 0,
    "points": [[0,0],[200,0]], "endArrowhead": "arrow",
    "start": { "id": "box1" }, "end": { "id": "box2" } }
  ```
Never: `startBinding` or `endBinding`; those are internal Excalidraw fields.

## 5. Place background zones first

### Use zones to group related elements
Zones are low-opacity rectangles that sit behind related elements. Place them first in the array. Add a standalone text label just inside the top-left corner.
Example:
  ```json
  { "type": "rectangle", "id": "zone1", "x": 80, "y": 80, "width": 540, "height": 400,
    "backgroundColor": "#d3f9d8", "fillStyle": "solid", "roundness": { "type": 3 },
    "strokeColor": "#22c55e", "strokeWidth": 1, "opacity": 35 }
  ```

## 6. Use the palette

### Pick fill colors by purpose
Use pastel fills for shape backgrounds: light blue `#a5d8ff` for input, sources, and primary; light green `#b2f2bb` for success, output, and completed; light orange `#ffd8a8` for warning, pending, and external; light purple `#d0bfff` for processing and middleware; light red `#ffc9c9` for error, critical, and alerts; light yellow `#fff3bf` for notes, Decisions, and planning; light teal `#c3fae8` for storage, data, and memory.

### Pick stroke colors by role
Use blue `#1971c2` for primary, green `#2f9e44` for success, purple `#6741d9` for accent, orange `#e8590c` for warning, red `#e03131` for error, teal `#0c8599` for data, and gray `#868e96` for neutral.

### Pick zone backgrounds by layer
Use blue zone `#dbe4ff` for frontend, purple zone `#e5dbff` for logic or Agent, and green zone `#d3f9d8` for data or tool. Zone opacity stays 30 to 35.
Never: invent a color outside this palette.

## 7. Use the complete push and append Example

### Push the first group, then append progressive additions
The first push establishes the zone, label, first shape, arrow, and second shape. Append adds the next arrow and shape.
Example:
  ```bash
  drawbridge push my-diagram '{"elements": [
    { "type": "rectangle", "id": "zone-fe", "x": 80, "y": 60, "width": 540, "height": 200,
      "backgroundColor": "#dbe4ff", "fillStyle": "solid", "roundness": { "type": 3 },
      "strokeColor": "#4a9eed", "strokeWidth": 1, "opacity": 35 },
    { "type": "text", "id": "zone-fe-label", "x": 100, "y": 66, "text": "Frontend",
      "fontSize": 16, "strokeColor": "#1971c2" },
    { "type": "rectangle", "id": "app", "x": 120, "y": 100, "width": 200, "height": 80,
      "roundness": { "type": 3 }, "backgroundColor": "#a5d8ff", "fillStyle": "solid",
      "label": { "text": "React App", "fontSize": 20 } },
    { "type": "arrow", "id": "a1", "x": 320, "y": 140, "width": 150, "height": 0,
      "points": [[0,0],[150,0]], "endArrowhead": "arrow",
      "start": { "id": "app" }, "end": { "id": "api" },
      "label": { "text": "REST API", "fontSize": 14 } },
    { "type": "rectangle", "id": "api", "x": 470, "y": 100, "width": 200, "height": 80,
      "roundness": { "type": 3 }, "backgroundColor": "#d0bfff", "fillStyle": "solid",
      "label": { "text": "API Server", "fontSize": 20 } }
  ]}'
  ```
Example:
  ```bash
  drawbridge append my-diagram '{"elements": [
    { "type": "arrow", "id": "a2", "x": 570, "y": 140, "width": 0, "height": 150,
      "points": [[0,0],[0,150]], "endArrowhead": "arrow",
      "start": { "id": "api" }, "end": { "id": "db" } },
    { "type": "rectangle", "id": "db", "x": 470, "y": 310, "width": 200, "height": 80,
      "roundness": { "type": 3 }, "backgroundColor": "#c3fae8", "fillStyle": "solid",
      "label": { "text": "Database", "fontSize": 20 } }
  ]}'
  ```

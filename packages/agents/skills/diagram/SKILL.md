---
name: diagram
description: |
  Generate Excalidraw diagrams with a hand-drawn aesthetic. Pushes elements to a
  live Excalidraw canvas via HTTP API, or renders to PNG/SVG.
---

# Diagram Skill

Generate hand-drawn style diagrams as Excalidraw JSON. Push to a live canvas via HTTP or render to PNG/SVG.

## Workflow

1. **Plan layout** — zones, flow direction, element grouping
2. **Generate elements** — follow drawing order below
3. **Push to live viewer** — use `drawbridge` CLI (see Output)
4. **Render to file** — PNG/SVG if needed

## Element Format

Only specify what matters. The browser fills in all internal properties automatically.

**Required fields:** `type`, `id` (unique string), `x`, `y`

**Defaults (skip these):** strokeColor="#1e1e1e", backgroundColor="transparent", fillStyle="solid", strokeWidth=2, roughness=1, opacity=100

### Labeled Shapes (preferred)

```json
{ "type": "rectangle", "id": "r1", "x": 100, "y": 100, "width": 160, "height": 80,
  "roundness": { "type": 3 }, "backgroundColor": "#a5d8ff", "fillStyle": "solid",
  "label": { "text": "API Server", "fontSize": 20 } }
```

- Works on rectangle, ellipse, diamond
- Text auto-centers; container only auto-resizes when width/height are omitted
- When setting explicit dimensions, calculate width from text: `max(120, text.length * fontSize * 0.6 + 40)`
- Always include `"roundness": { "type": 3 }` for rounded corners

### Standalone Text

```json
{ "type": "text", "id": "t1", "x": 150, "y": 50, "text": "System Architecture", "fontSize": 28 }
```

- `x` is the LEFT edge — to center at cx: `x = cx - (text.length * fontSize * 0.5) / 2`
- `textAlign` only affects multi-line wrapping, not position

### Arrows

```json
{ "type": "arrow", "id": "a1", "x": 300, "y": 150, "width": 200, "height": 0,
  "points": [[0,0],[200,0]], "endArrowhead": "arrow" }
```

- `points`: [dx, dy] offsets from element x,y
- `endArrowhead`: null | "arrow" | "bar" | "dot" | "triangle"
- Add `"label": { "text": "API call", "fontSize": 16 }` for arrow labels

### Arrow Bindings

Bind arrows to shapes so they stay connected:

```json
{ "type": "arrow", "id": "a1", "x": 300, "y": 150, "width": 200, "height": 0,
  "points": [[0,0],[200,0]], "endArrowhead": "arrow",
  "start": { "id": "box1" }, "end": { "id": "box2" } }
```

Use `start`/`end` (skeleton format), NOT `startBinding`/`endBinding` (internal format). Position the arrow's x,y at the right edge of the source shape for left-to-right connections.

### Background Zones

Low-opacity rectangles to group related elements. Place FIRST in array (z-order: first = back).

```json
{ "type": "rectangle", "id": "zone1", "x": 80, "y": 80, "width": 540, "height": 400,
  "backgroundColor": "#d3f9d8", "fillStyle": "solid", "roundness": { "type": 3 },
  "strokeColor": "#22c55e", "strokeWidth": 1, "opacity": 35 }
```

Add a zone label as standalone text just inside the top-left corner.

## Color Palette

**Fills (pastel, for shape backgrounds):**
- **Light Blue** `#a5d8ff` — input, sources, primary
- **Light Green** `#b2f2bb` — success, output, completed
- **Light Orange** `#ffd8a8` — warning, pending, external
- **Light Purple** `#d0bfff` — processing, middleware
- **Light Red** `#ffc9c9` — error, critical, alerts
- **Light Yellow** `#fff3bf` — notes, decisions, planning
- **Light Teal** `#c3fae8` — storage, data, memory

**Strokes (borders, text):**
- **Blue** `#1971c2` — primary
- **Green** `#2f9e44` — success
- **Purple** `#6741d9` — accent
- **Orange** `#e8590c` — warning
- **Red** `#e03131` — error
- **Teal** `#0c8599` — data
- **Gray** `#868e96` — neutral

**Zone backgrounds (use with opacity: 30-35):**
- **Blue zone** `#dbe4ff` — UI / frontend layer
- **Purple zone** `#e5dbff` — logic / agent layer
- **Green zone** `#d3f9d8` — data / tool layer

## Sizing Rules

- **Font:** min 16 body, min 20 titles, min 14 annotations only. Never below 14.
- **Label width:** `max(120, text.length * fontSize * 0.6 + 40)` — always size boxes to fit their text
- **Elements:** min height 60 for labeled shapes. 20-30px gaps between elements.
- Prefer fewer, larger elements over many small ones.

## Drawing Order

Array order = z-order (first = back, last = front).

Emit progressively: `zone → shape → its arrows → next shape → its arrows`

- GOOD: `zone → box1 → arrow1 → box2 → arrow2 → box3`
- BAD: `box1 → box2 → box3 → arrow1 → arrow2`

## Complete Example

Two connected boxes with a zone:

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

Then add more elements progressively:

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

## Output

Use the `drawbridge` CLI. It auto-starts the server if not running.

```bash
drawbridge push <session> '{"elements": [...]}'    # replace all elements
drawbridge append <session> '{"elements": [...]}'  # add to existing
drawbridge clear <session>                         # clear canvas
drawbridge undo <session>                          # undo last operation
drawbridge get <session>                           # get current elements
drawbridge open <session>                          # open viewer in browser
drawbridge render input.excalidraw output.png      # render to PNG/SVG
```

### Save as .excalidraw file

```json
{ "type": "excalidraw", "version": 2, "source": "https://excalidraw.com",
  "elements": [], "appState": { "viewBackgroundColor": "#ffffff", "gridSize": null }, "files": {} }
```

## Checklist

- [ ] Drawing order correct? (zones → shapes → arrows, progressive)
- [ ] All font sizes ≥ 14?
- [ ] All labeled shapes wide enough for text? (`max(120, text.length * fontSize * 0.6 + 40)`)
- [ ] All labeled shapes ≥ 60 height?
- [ ] Arrow bindings use `start`/`end` (not `startBinding`/`endBinding`)?
- [ ] Colors from the palette above?
- [ ] Pushed to live viewer or rendered to file?

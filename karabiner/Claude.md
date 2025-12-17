---
description: Keyboard configuration using karabiner.ts - pipeline-based generator for HRM + layers
globs:
  - "**/*.ts"
  - "**/karabiner.json"
---

# Karabiner Keyboard Config

TypeScript-based keyboard configuration using [karabiner.ts](https://github.com/evan-liu/karabiner.ts).

## Architecture

**Pipeline-based generator** - compose transformers for clean separation:

```
src/
├── types.ts              # Shared types (KeyboardConfig, PipelineContext, Transformer)
├── layout.ts             # parseLayout(), isOppositeHand()
├── generator.ts          # Main: compose pipeline for each key
├── pipeline/
│   ├── index.ts          # pipe() combinator
│   ├── layers.ts         # Layer triggers + bindings
│   ├── hrm.ts            # Home row mods (activation, rollover, source)
│   └── default.ts        # Fallback (bare key passthrough)
└── utils/
    ├── streak.ts         # streakUpdates() - typing streak detection
    └── combos.ts         # allCombinations() - 2^n modifier combos
```

**Flow:** For each key → `addLayers(ctx) → addHrm(ctx) → addDefault(ctx)`

## Key Concepts

### Pipeline Pattern

**Transformer:** `(ctx: PipelineContext) => Manipulator[]`

Each transformer:
- Receives context (key, config, keyboard layout)
- Returns manipulator array (priority order)
- Earlier results = higher priority in Karabiner

```typescript
// generator.ts
for (const key of allKeys) {
  const ctx = { key, config, kb }

  allManipulators.push(
    ...addLayers(ctx),   // 1. Layer triggers + bindings (highest priority)
    ...addHrm(ctx),      // 2. HRM behaviors
    ...addDefault(ctx),  // 3. Fallback (bare key)
  )
}
```

### Home Row Mods (HRM)

**Dual protection against accidental activation:**

1. **Hold threshold:** 120ms before HRM becomes ready
2. **Typing streak:** Active within window (default 120ms) blocks HRM activation

**Three behaviors:**

- **Activation:** Opposite-hand key pressed while HRM held + ready + no typing streak → apply modifier
- **Rollover:** Key pressed before HRM ready → output held letter first, then continue
- **Source:** HRM key behavior (tap alone → letter, hold → set vars, held threshold → ready)

**Example:** Hold `a` (⌃), press `j` within 120ms → outputs "aj" (rollover, not Ctrl+J)

### Layers

**Trigger:** Key that activates layer when held (after 120ms threshold), outputs itself when tapped alone

**Dual protection:** Like HRM - hold threshold (120ms) before layer activates, tap outputs trigger key

**Bindings:** Key mappings active when layer is on

**Swallowing:** Undefined keys output nothing when layer active (no passthrough)

**Inheritance:** When `inherit: true`, child layer uses parent bindings for unbound keys

**Parent/Child:** Sub-layers require parent active

**Handoff:** When `true` (default), trigger tap alone keeps layer active after parent released

**HRM Rollover:** Layer triggers check if HRM key is held but not ready - outputs the letter before activating layer

## Config Example

```typescript
const config: KeyboardConfig = {
  layout: [
    'q w e r t | y u i o p',
    'a s d f g | h j k l ;',
    'z x c v b | n m , . /',
  ],

  hrm: {
    a: '⌃', s: '⌘', d: '⇧', f: '⌥',  // Left hand
    j: '⌥', k: '⇧', l: '⌘', ';': '⌃', // Right hand
  },

  layers: {
    nav: {
      trigger: 'spacebar',
      bindings: {
        j: 'left_arrow',
        k: 'down_arrow',
        l: 'right_arrow',
        i: 'up_arrow',
      },
    },
    window: {
      trigger: 'w',
      parent: 'nav',
      bindings: {
        j: ['left_arrow', '⌃'],
        l: ['right_arrow', '⌃'],
      },
    },
  },

  streakWindowMs: 120,           // Typing streak window
  hrmHoldThresholdMs: 120,       // HRM activation delay
  layerHoldThresholdMs: 120,     // Layer activation delay
}

writeToProfile('MyProfile', generateKeyboardRules(config))
```

## Implementation Details

### Typing Streak Detection

Uses Karabiner v15.5+ expressions for timestamp-based tracking (no race conditions).

**Variables:**
- `isTypingStreak` (0 or 1) - true if within streak window
- `streakExpiry` (milliseconds) - timestamp when streak expires

**Every key press includes:**
```typescript
streakUpdates(windowMs) // Sets isTypingStreak and updates expiry
```

### Layer Variable Lifecycle

**Trigger key pressed:**
- Set `{layerName}Active = 0` (inactive initially)
- Track typing streak

**After hold threshold (120ms):**
- Set `{layerName}Active = 1` (via toIfHeldDown)

**Trigger key released:**
- Set `{layerName}Active = 0` (via toAfterKeyUp)

**Tap alone:** Output trigger key (via toIfAlone)

### HRM Variable Lifecycle

**HRM key pressed:**
- Set `{key}Held = 1`
- Set `{key}HeldReady = 0` (not ready yet)
- Set `{key}Outputted = 0`

**After hold threshold (120ms):**
- Set `{key}HeldReady = 1` (via toIfHeldDown)

**Key released:**
- Reset all three variables to 0 (via toAfterKeyUp)

**Tap alone:** Output the letter (via toIfAlone)

### Modifier Combinations

Uses `allCombinations()` to generate all 2^n HRM modifier combos, sorted most-specific first.

Example: Left hand holds `s` (⌘) + `d` (⇧), press right-hand `j` → Cmd+Shift+J

Order matters - Karabiner checks rules top to bottom, first match wins.

## See Also

- [karabiner.ts API Reference](./docs/karabiner-ts-reference.md) - Full karabiner.ts DSL documentation

# Karabiner.ts API Reference

TypeScript DSL for [Karabiner-Elements](https://karabiner-elements.pqrs.org/) keyboard remapping.

## Quick Start

```typescript
import { writeToProfile, rule, map, layer } from 'karabiner.ts'

writeToProfile('Default', [
  rule('Basic Mapping').manipulators([
    map('⇪').to('⎋'),  // Caps Lock → Escape
  ]),
])
```

## Core Functions

### writeToProfile()
Write rules to Karabiner profile at `~/.config/karabiner/karabiner.json`

```typescript
// Basic usage
writeToProfile('Default', [...rules])

// Dry run (print JSON to console)
writeToProfile('--dry-run', [...rules])

// Custom path
writeToProfile({
  name: 'MyProfile',
  karabinerJsonPath: '/custom/path/karabiner.json'
}, [...rules])

// Global parameters
writeToProfile('Default', [...rules], {
  'basic.simultaneous_threshold_milliseconds': 100,
  'simlayer.threshold_milliseconds': 150,
  'double_tap.delay_milliseconds': 200,
  'duo_layer.delay_milliseconds': 150,
})
```

### rule()
Group manipulators with shared conditions

```typescript
// Basic rule
rule('Number Keys').manipulators([
  map(1).to(2),
  map(2).to(3),
])

// With conditions
rule('Finder Keys', ifApp('com.apple.finder')).manipulators([
  map('h').to('←'),
  map('l').to('→'),
])

// Method chaining
rule('Variable Condition')
  .condition(ifVar('vim-mode'))
  .manipulators([
    map('j').to('↓'),
    map('k').to('↑'),
  ])
```

### map()
Define key mappings

```typescript
// Basic
map('a').to('b')

// With mandatory modifiers
map(',', 'left_command')
map(1, '⌘', '⇪')

// Left/right specific modifiers
map('←', { right: '⌘⌥' })

// Optional modifiers
map('left_command', { optional: '⇧' })
map('keypad_asterisk', 'optionalAny')  // or '??'

// Full event definition
map({
  key_code: 'a',
  modifiers: {
    mandatory: ['left_command', 'shift'],
    optional: ['caps_lock']
  }
})
```

## Modifier Symbols

| Symbol | Modifier | Full Name |
|--------|----------|-----------|
| ⌘ | command | Command |
| ⌥ | option | Option/Alt |
| ⌃ | control | Control |
| ⇧ | shift | Shift |
| ⇪ | caps_lock | Caps Lock |
| fn | fn | Function |

Special modifiers:
- `Hyper` = ⌘⌥⌃⇧
- `Meh` = ⌥⌃⇧
- `SuperHyper` = ⌘⌥⌃⇧fn

## Key Aliases

```
↑  up_arrow        ⏎  return_or_enter
↓  down_arrow      ⎋  escape
←  left_arrow      ⌫  delete_or_backspace
→  right_arrow     ⌦  delete_forward
⇞  page_up         ⇥  tab
⇟  page_down       ␣  spacebar
↖  home            -  hyphen
↘  end             =  equal_sign
                   [  open_bracket
                   ]  close_bracket
                   \  backslash
                   ;  semicolon
                   '  quote
                   `  grave_accent_and_tilde
                   ,  comma
                   .  period
                   /  slash
```

## Output Functions (to*)

### Basic Output
```typescript
// Single key
map('a').to('b')

// Key with modifiers
map('a').to('b', '⌘⇧')

// Multiple outputs (sequential)
map(1).to('a').to('b').to('c')

// Using toKey helper
map('a').to(toKey('b', '⌘⇧'))
```

### Special Modifiers
```typescript
map('⇪').toHyper()       // ⌘⌥⌃⇧
map('⇪').toMeh()         // ⌥⌃⇧
map('⇪').toSuperHyper()  // ⌘⌥⌃⇧fn
map('a').toNone()        // vk_none (disable key)
```

### Conditional Output
```typescript
// If tapped alone
map('⇪').toHyper().toIfAlone('⎋')

// If held down
map('a').to('b').toIfHeldDown('c')

// After key up
map('a').to('b').toAfterKeyUp('c')

// Delayed action
map('a').toDelayedAction(toKey('x'), toKey('y'))
```

### Shell Commands
```typescript
// Open application
map(1).toApp('Arc')
map(2).toApp('Finder')

// Paste text via clipboard
map('a').toPaste('✨')
```

### Other Output
```typescript
toConsumerKey('play_or_pause')
toInputSource({ language: 'en' })
toSetVar('mode', 1)
toNotificationMessage('id', 'message')
toRemoveNotificationMessage('id')
toMouseKey({ x: 100, y: 100 })
toStickyModifier('⌘')
toSleepSystem()
```

## Layers

### layer()
Activate mappings when key is held

```typescript
// Basic layer
layer('a', 'a-mode').manipulators([
  map(1).to(2),  // Hold 'a', press '1' → '2'
])

// Multiple trigger keys
layer(['a', ';'], 'nav-mode').manipulators([
  map('h').to('←'),
  map('j').to('↓'),
  map('k').to('↑'),
  map('l').to('→'),
])

// With modifiers
layer('a', 'a-mode')
  .modifiers('⌘')
  .manipulators([
    map(1).to(2),  // ⌘+a (hold), press 1 → 2
  ])

// Custom tap-alone behavior
layer('a', 'a-mode')
  .configKey((v) => v.toIfAlone('b', '⌘'), true)
  .manipulators([
    map(1).to(2),  // Hold 'a' + 1 → 2; tap 'a' alone → ⌘b
  ])

// Pass modifiers through
layer(';', 'nav-layer')
  .modifiers('??')  // or 'optionalAny'
  .manipulators([
    map('l').to('→'),  // ⌘ ; + l → ⌘ →
  ])
```

### simlayer()
Activate via simultaneous key press within threshold time

```typescript
// Basic simlayer
simlayer('a', 'a-mode').manipulators([
  map(1).to(','),
  map(2).to('.'),
])

// Custom threshold (default 200ms)
simlayer('a', 'a-mode', 100).manipulators([
  map(1).to(2),
])

// Custom modifiers (default: optional 'any')
simlayer('a', 'a-mode')
  .modifiers({ optional: '⇪' })
  .manipulators([
    map(1).to(2),
  ])

// Custom simultaneous options
simlayer('s', 's-mode')
  .options({
    detect_key_down_uninterruptedly: false,
    key_down_order: 'insensitive',
  })
  .manipulators([
    map('d').to('f'),
  ])
```

## Conditions

### Application Conditions
```typescript
// Active if frontmost app matches
ifApp('com.apple.finder')
ifApp('Finder')  // Short name

// Bundle identifiers
ifApp({ bundle_identifiers: ['com.apple.Safari'] })

// File paths
ifApp({ file_paths: ['/Applications/Chrome.app'] })

// Unless (inverse)
ifApp('Finder').unless()
```

### Device Conditions
```typescript
// Specific device
ifDevice({ product_id: 123 })
ifDevice({ vendor_id: 1452, product_id: 123 })

// Device exists
ifDeviceExists({ vendor_id: 1452 })

// Unless
ifDevice({ product_id: 456 }).unless()
```

### Variable Conditions
```typescript
// Variable equals value
ifVar('a-mode', 1)
ifVar('mode', 'navigation')
ifVar('enabled', true)

// Variable not equals
ifVar('a-mode', 1).unless()

// Setting variables
map('a')
  .to(toSetVar('nav-mode', 1))
  .toNotificationMessage('hint', 'Nav mode on')
```

### Other Conditions
```typescript
// Input source
ifInputSource({ language: 'en' })
ifInputSource({ input_source_id: 'com.apple.keylayout.US' })

// Keyboard type
ifKeyboardType(['ansi'])

// Event changed
ifEventChanged(true)
```

## Advanced Features

### mapDoubleTap()
Detect double-tap gestures

```typescript
// Basic double tap
mapDoubleTap('↑').to('↖︎')  // Double tap up → home

// Custom single tap behavior
mapDoubleTap('⇪')
  .to('⎋')  // Double tap → escape
  .singleTap(toKey('q', '⌘'))  // Single tap → ⌘q

// Disable single tap
mapDoubleTap('q', '⌘')
  .to('q', '⌘')
  .singleTap(null)  // Must double-tap ⌘q to quit

// Custom delay (default 200ms)
mapDoubleTap('⇪', 150)
  .to('⎋')
  .delay(150)
```

### mapSimultaneous()
Detect simultaneous key presses

```typescript
// Basic simultaneous
mapSimultaneous(['a', 's']).to('⎋')

// With pointing button
mapSimultaneous(['a', { pointing_button: 'button1' }])
  .to('escape')

// With options and threshold
mapSimultaneous(
  ['a', 'b'],
  { key_down_order: 'strict' },
  100  // milliseconds
).to('c')

// With modifiers
mapSimultaneous(['j', 'k'])
  .modifiers('⌘')
  .to('⎋')
```

### mapConsumerKey()
Map media/consumer keys

```typescript
mapConsumerKey('play_or_pause').to('a')
mapConsumerKey('volume_increment')
mapConsumerKey('volume_decrement')
mapConsumerKey('menu', '⌘', 'any')
```

### mapPointingButton()
Map mouse buttons

```typescript
mapPointingButton('button1', '⌘')
```

## Utility Functions

### withMapper()
Reduce duplication when creating multiple manipulators

```typescript
// Map array items
withMapper(['⌘', '⌥', '⌃', '⇧', '⇪'])((k, i) =>
  map((i + 1) as NumberKeyValue).toPaste(k)
)

// Map object entries
withMapper({
  c: 'Calendar',
  f: 'Finder',
  s: 'Safari',
})((key, appName) =>
  map(key, 'Meh').toApp(appName)
)

// Using 'as const' for better type inference
withMapper({
  h: '←',
  j: '↓',
  k: '↑',
  l: '→',
} as const)((from, to) => map(from).to(to))
```

### withModifier()
Apply modifiers to group of manipulators

```typescript
withModifier('optionalAny')([
  map(1, '⌘').to('a'),
  map(1, '⌥').to('b'),
])

withModifier(['left_control', 'left_option'])([
  map('a').to('x'),
])

withModifier('‹⌃⌥')([
  map('b').to('y'),
])
```

### withCondition()
Apply condition to group of manipulators

```typescript
withCondition(ifDevice({ product_id: 1 }))([
  map('a').to('x'),
  map('b').to('y'),
])

withCondition(ifApp('Finder'))([
  map('h').to('←'),
  map('l').to('→'),
])
```

## Import Functions

```typescript
import { importProfile, importJson } from 'karabiner.ts'
import { resolve } from 'node:path'
import { homedir } from 'node:os'

// Import rules from another profile
writeToProfile('karabiner.ts', [
  rule('New Rules').manipulators([...]),
  importProfile('ByAnotherTool'),
])

// Import JSON files
writeToProfile('Default', [
  importJson(resolve(homedir(), '.config/karabiner/assets/file.json')),
  importJson(resolve(__dirname, './my-rules.json')),
])
```

## Common Patterns

### Caps Lock → Hyper (tap for Escape)
```typescript
rule('Hyper Key').manipulators([
  map('⇪').toHyper().toIfAlone('⎋'),
])
```

### Navigation Layer
```typescript
layer(';', 'nav-layer').manipulators([
  map('h').to('←'),
  map('j').to('↓'),
  map('k').to('↑'),
  map('l').to('→'),
  map('u').to('⇞'),  // page up
  map('d').to('⇟'),  // page down
])
```

### App Launcher
```typescript
rule('Meh + Key = Launch App').manipulators([
  withMapper({
    c: 'Calendar',
    f: 'Finder',
    m: 'Mail',
    s: 'Safari',
    t: 'Terminal',
  })((key, app) =>
    map(key, 'Meh').toApp(app)
  ),
])
```

### App-Specific Mappings
```typescript
rule('VSCode Vim', ifApp('com.microsoft.VSCode')).manipulators([
  map('j', '⌃').to('↓'),
  map('k', '⌃').to('↑'),
])
```

### Dual-Function Keys
```typescript
// Tap for key, hold for modifier
map('a').to('a').toIfHeldDown('left_command')

// Layer activation with tap-alone behavior
layer('a', 'a-mode')
  .configKey((v) => v.toIfAlone('a'), true)
  .manipulators([
    map('s').to('d'),
  ])
```

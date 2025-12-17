import { layer, map, writeToProfile } from 'karabiner.ts'
import { generateKeyboardRules, type KeyboardConfig } from './generator'

// =============================================================================
// Existing caps layer (preserved for daily use)
// =============================================================================

const capsLayer = layer('caps_lock', 'caps_lock pressed')
  .modifiers('??') // Allow other modifiers (shift for selection, etc.)
  .manipulators([
    map('j').to('left_arrow'),
    map('k').to('down_arrow'),
    map('l').to('right_arrow'),
    map('i').to('up_arrow'),
    map('h').to('left_arrow', '⌥'),
    map(';').to('right_arrow', '⌥'),
    map('u').to('left_arrow', '⌘'),
    map('o').to('right_arrow', '⌘'),
    map('spacebar').to('escape'),
    map('d').to('d', '⌘'),
    map('s').to('s', '⌃'),
    map('delete_or_backspace').to('delete_forward'),
  ])

// =============================================================================
// Test Config - Unified Keyboard System
// =============================================================================

const config: KeyboardConfig = {
  layout: [
    'q w e r t | y u i o p',
    'a s d f g | h j k l ;',
    'z x c v b | n m , . /',
  ],

  hrm: {
    // Left hand
    a: '⌃',
    s: '⌘',
    d: '⇧',
    f: '⌥',
    // Right hand
    j: '⌥',
    k: '⇧',
    l: '⌘',
    ';': '⌃',
  },

  layers: {
    nav: {
      trigger: 'spacebar',
      bindings: {
        j: 'left_arrow',
        k: 'down_arrow',
        l: 'right_arrow',
        i: 'up_arrow',
        u: 'page_up',
        d: 'page_down',
      },
    },
    window: {
      trigger: 'w',
      parent: 'nav',
      bindings: {
        j: ['left_arrow', '⌃'],
        k: ['down_arrow', '⌃'],
        l: ['right_arrow', '⌃'],
        i: ['up_arrow', '⌃'],
      },
    },
    navExtended: {
      trigger: 'e',
      parent: 'nav',
      inherit: true,
      bindings: {
        u: 'page_up',
        d: 'page_down',
      },
    },
    navShift: {
      trigger: 'r',
      parent: 'nav',
      handoff: false,
      bindings: {
        j: ['left_arrow', '⇧'],
        k: ['down_arrow', '⇧'],
        l: ['right_arrow', '⇧'],
        i: ['up_arrow', '⇧'],
      },
    },
  },

  streakWindowMs: 100, // ~105 WPM threshold
  hrmHoldThresholdMs: 120,
  layerHoldThresholdMs: 120,
}

// =============================================================================
// Write Profiles
// =============================================================================

writeToProfile('Default profile', [capsLayer])
writeToProfile('Testing', generateKeyboardRules(config))

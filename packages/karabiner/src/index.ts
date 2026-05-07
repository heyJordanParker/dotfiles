import { layer, map, writeToProfile } from 'karabiner.ts'

const capsLayer = layer('caps_lock', 'caps_lock pressed').manipulators([
  // Arrows (ijkl)
  map('j').to('left_arrow'),
  map('k').to('down_arrow'),
  map('l').to('right_arrow'),
  map('i').to('up_arrow'),

  // Word navigation
  map('h').to('left_arrow', '⌥'),
  map(';').to('right_arrow', '⌥'),

  // Line navigation
  map('u').to('left_arrow', '⌘'),
  map('o').to('right_arrow', '⌘'),

  // Actions
  map('spacebar').to('escape'),
  map('d').to('d', '⌘'),
  map('s').to('s', '⌃'),
  map('delete_or_backspace').to('delete_forward'),
])

writeToProfile('Default profile', [capsLayer])

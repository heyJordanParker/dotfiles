// Visual layout - easy to read and edit
const layout = [
  'q w e r t | y u i o p',
  'a s d f g | h j k l ;',
  'z x c v b | n m , . /',
] as const

// Metadata overlays - only where needed
export const modifiers = {
  a: '⌃',
  s: '⌘',
  d: '⇧',
  f: '⌥',
  j: '⌥',
  k: '⇧',
  l: '⌘',
  ';': '⌃',
} as const

// Future: custom shift layer overrides
export const shiftOverrides = {
  ',': '<',
  '.': '>',
  '/': '?',
} as const

// Parse once, derive everything
function parseLayout(lines: readonly string[]) {
  const left: string[] = []
  const right: string[] = []
  const rows: string[][] = []

  for (const line of lines) {
    const [l, r] = line.split('|').map((s) => s.trim().split(/\s+/))
    left.push(...l)
    right.push(...r)
    rows.push([...l, ...r])
  }

  return {
    left,
    right,
    all: [...left, ...right],
    homeRow: rows[1],
    rows,
  } as const
}

export const keyboard = parseLayout(layout)

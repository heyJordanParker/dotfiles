import type { ParsedKeyboard } from './types'

/**
 * Parse visual keyboard layout into left/right hand groups.
 * Format: "q w e r t | y u i o p" (pipe separates hands)
 */
export function parseLayout(lines: string[]): ParsedKeyboard {
  const left: string[] = []
  const right: string[] = []

  for (const line of lines) {
    const [l, r] = line.split('|').map((s) => s.trim().split(/\s+/))
    left.push(...l)
    right.push(...r)
  }

  return { left, right, all: [...left, ...right] }
}

/**
 * Check if two keys are on opposite hands.
 */
export function isOppositeHand(key: string, otherKey: string, kb: ParsedKeyboard): boolean {
  const keyIsLeft = kb.left.includes(key)
  const otherIsLeft = kb.left.includes(otherKey)
  return keyIsLeft !== otherIsLeft
}

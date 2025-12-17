/**
 * Generate all non-empty combinations of items (2^n - 1 combinations).
 * Returns in descending order by count (most items first = highest priority).
 */
export function allCombinations<T>(items: T[]): T[][] {
  const n = items.length
  const result: T[][] = []

  // Generate all 2^n - 1 combinations (excluding empty set)
  // Process in descending order so more-specific combinations come first
  for (let mask = (1 << n) - 1; mask > 0; mask--) {
    const combo = items.filter((_, i) => mask & (1 << i))
    result.push(combo)
  }

  return result
}

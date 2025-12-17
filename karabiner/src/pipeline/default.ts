import { map } from 'karabiner.ts'
import type { PipelineContext, Transformer } from '../types'
import { streakUpdates } from '../utils/streak'

/**
 * Only match bare key - lets Ctrl+X, Cmd+X etc. pass through to OS.
 */
export function bareKeyMap(key: string) {
  return map({ key_code: key, modifiers: { mandatory: [], optional: [] } } as any)
}

/**
 * Default transformer - fallback behavior for keys.
 * Outputs the key with streak tracking.
 */
export const addDefault: Transformer = (ctx) => {
  const { key, config } = ctx
  const windowMs = config.streakWindowMs ?? 150
  const streak = streakUpdates(windowMs)

  return [
    bareKeyMap(key)
      .to(streak)
      .to(key as any)
  ]
}

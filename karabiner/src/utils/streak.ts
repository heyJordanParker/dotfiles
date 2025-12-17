import type { ToEvent } from 'karabiner.ts'

/**
 * Generate streak tracking variable updates.
 * Every key press should include these to maintain typing streak detection.
 */
export function streakUpdates(windowMs: number): ToEvent[] {
  return [
    {
      set_variable: {
        name: 'isTypingStreak',
        expression: 'if(streakExpiry > system.now.milliseconds, 1, 0)',
      },
    } as ToEvent,
    {
      set_variable: {
        name: 'streakExpiry',
        expression: `system.now.milliseconds + ${windowMs}`,
      },
    } as ToEvent,
  ]
}

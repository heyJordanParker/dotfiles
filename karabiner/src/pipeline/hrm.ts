import { ifVar, map, toSetVar, type ToEvent } from 'karabiner.ts'
import type { PipelineContext, Transformer, ModifierKey } from '../types'
import { isOppositeHand } from '../layout'
import { streakUpdates } from '../utils/streak'
import { allCombinations } from '../utils/combos'
import { bareKeyMap } from './default'

/**
 * Check if a key is an HRM source key.
 */
function isHrmKey(key: string, config: PipelineContext['config']): boolean {
  return key in (config.hrm ?? {})
}

/**
 * Get opposite-hand HRM keys for a given key.
 */
function getOppositeHrmKeys(
  key: string,
  config: PipelineContext['config'],
  kb: PipelineContext['kb']
): { key: string; modifier: ModifierKey }[] {
  const hrm = config.hrm ?? {}
  return Object.entries(hrm)
    .filter(([hrmKey]) => hrmKey !== key && isOppositeHand(key, hrmKey, kb))
    .map(([k, mod]) => ({ key: k, modifier: mod }))
}

/**
 * Get same-hand HRM keys for a given key.
 */
function getSameHandHrmKeys(
  key: string,
  config: PipelineContext['config'],
  kb: PipelineContext['kb']
): string[] {
  const hrm = config.hrm ?? {}
  return Object.keys(hrm).filter(
    (hrmKey) => hrmKey !== key && !isOppositeHand(key, hrmKey, kb)
  )
}

/**
 * Generate HRM activation rules (opposite-hand modifier combos).
 * When opposite-hand HRM keys are held and ready, apply combined modifiers.
 */
function addHrmActivation(ctx: PipelineContext): ReturnType<typeof bareKeyMap>[] {
  const { key, config, kb } = ctx
  const windowMs = config.streakWindowMs ?? 150
  const streak = streakUpdates(windowMs)
  const manipulators: ReturnType<typeof bareKeyMap>[] = []

  const oppositeHrmKeys = getOppositeHrmKeys(key, config, kb)
  if (oppositeHrmKeys.length === 0) return []

  // Generate all HRM combinations (most-specific first)
  // HRM activates when: Ready=1 (held past threshold) AND not in typing streak
  for (const activeHrm of allCombinations(oppositeHrmKeys)) {
    const readyConditions = [
      ifVar('isTypingStreak', 0),  // Not in typing streak
      ...activeHrm.flatMap((h) => [
        ifVar(`${h.key}Held`, 1),
        ifVar(`${h.key}HeldReady`, 1),
      ]),
    ]

    const combinedMods = activeHrm.map((h) => h.modifier).join('')

    manipulators.push(
      bareKeyMap(key)
        .condition(...readyConditions)
        .to(streak)
        .to(key as any, combinedMods as any)
    )
  }

  return manipulators
}

/**
 * Generate HRM rollover rules (both opposite and same hand).
 * When HRM is held but not ready/outputted, output the held key first.
 */
function addHrmRollover(ctx: PipelineContext): ReturnType<typeof bareKeyMap>[] {
  const { key, config, kb } = ctx
  const windowMs = config.streakWindowMs ?? 150
  const streak = streakUpdates(windowMs)
  const manipulators: ReturnType<typeof bareKeyMap>[] = []

  // Opposite-hand rollover
  const oppositeHrmKeys = getOppositeHrmKeys(key, config, kb)
  for (const hrm of oppositeHrmKeys) {
    const heldVar = `${hrm.key}Held`
    const readyVar = `${hrm.key}HeldReady`
    const outputtedVar = `${hrm.key}Outputted`

    // HRM held, not ready, not outputted → rollover
    manipulators.push(
      bareKeyMap(key)
        .condition(ifVar(heldVar, 1), ifVar(readyVar, 0), ifVar(outputtedVar, 0))
        .to(streak)
        .to(hrm.key as any)
        .to(key as any)
        .to(toSetVar(outputtedVar, 1) as ToEvent)
    )

    // HRM held and already outputted → just output target
    manipulators.push(
      bareKeyMap(key)
        .condition(ifVar(heldVar, 1), ifVar(outputtedVar, 1))
        .to(streak)
        .to(key as any)
    )
  }

  // Same-hand rollover (never activates modifier)
  const sameHandHrmKeys = getSameHandHrmKeys(key, config, kb)
  for (const hrmKey of sameHandHrmKeys) {
    const heldVar = `${hrmKey}Held`
    const outputtedVar = `${hrmKey}Outputted`

    // Not yet outputted → output HRM key then this key, mark outputted
    manipulators.push(
      bareKeyMap(key)
        .condition(ifVar(heldVar, 1), ifVar(outputtedVar, 0))
        .to(streak)
        .to(hrmKey as any)
        .to(key as any)
        .to(toSetVar(outputtedVar, 1) as ToEvent)
    )

    // Already outputted → just output this key
    manipulators.push(
      bareKeyMap(key)
        .condition(ifVar(heldVar, 1), ifVar(outputtedVar, 1))
        .to(streak)
        .to(key as any)
    )
  }

  return manipulators
}

/**
 * Generate HRM source rules (for HRM keys themselves).
 * Handles: modifier pass-through, HRM-to-HRM rollover, source behavior.
 */
function addHrmSource(ctx: PipelineContext): ReturnType<typeof bareKeyMap>[] {
  const { key, config, kb } = ctx
  const windowMs = config.streakWindowMs ?? 150
  const streak = streakUpdates(windowMs)
  const manipulators: ReturnType<typeof bareKeyMap>[] = []

  const heldVar = `${key}Held`
  const readyVar = `${key}HeldReady`
  const outputtedVar = `${key}Outputted`

  // HRM target behavior: this key is a target for opposite-hand HRM keys
  const oppositeHrmKeys = getOppositeHrmKeys(key, config, kb)

  if (oppositeHrmKeys.length > 0) {
    // Generate all HRM combinations (most-specific first)
    // HRM activates when: Ready=1 (held past threshold) AND not in typing streak
    for (const activeHrm of allCombinations(oppositeHrmKeys)) {
      const readyConditions = [
        ifVar('isTypingStreak', 0),  // Not in typing streak
        ...activeHrm.flatMap((h) => [
          ifVar(`${h.key}Held`, 1),
          ifVar(`${h.key}HeldReady`, 1),
        ]),
      ]

      const combinedMods = activeHrm.map((h) => h.modifier).join('')

      manipulators.push(
        bareKeyMap(key)
          .condition(...readyConditions)
          .to(streak)
          .to(key as any, combinedMods as any)
      )
    }

    // Rollover for opposite-hand HRM keys
    for (const h of oppositeHrmKeys) {
      manipulators.push(
        bareKeyMap(key)
          .condition(ifVar(`${h.key}Held`, 1), ifVar(`${h.key}HeldReady`, 0), ifVar(`${h.key}Outputted`, 0))
          .to(streak)
          .to(h.key as any)
          .to(key as any)
          .to(toSetVar(`${h.key}Outputted`, 1) as ToEvent)
      )

      manipulators.push(
        bareKeyMap(key)
          .condition(ifVar(`${h.key}Held`, 1), ifVar(`${h.key}Outputted`, 1))
          .to(streak)
          .to(key as any)
      )
    }
  }

  // Pass-through for system modifiers (Ctrl+A → Ctrl+A, etc.)
  for (const mod of ['⌃', '⌘', '⇧', '⌥'] as const) {
    manipulators.push(
      map(key as any, mod).to(key as any, mod) as any
    )
  }

  // Same-hand HRM-to-HRM rollover
  const sameHandHrmKeys = getSameHandHrmKeys(key, config, kb)

  for (const otherHrm of sameHandHrmKeys) {
    const otherHeldVar = `${otherHrm}Held`
    const otherOutputtedVar = `${otherHrm}Outputted`

    // Other HRM held, not yet outputted → output it, then this key
    manipulators.push(
      bareKeyMap(key)
        .condition(ifVar(otherHeldVar, 1), ifVar(otherOutputtedVar, 0))
        .to(streak)
        .to(otherHrm as any)
        .to(key as any)
        .to(toSetVar(otherOutputtedVar, 1) as ToEvent)
    )

    // Other HRM held, already outputted → just output this key
    manipulators.push(
      bareKeyMap(key)
        .condition(ifVar(otherHeldVar, 1), ifVar(otherOutputtedVar, 1))
        .to(streak)
        .to(key as any)
    )
  }

  // HRM source behavior for bare key
  // Ready=0 initially, becomes Ready=1 after hold threshold (not immediately)
  const holdThresholdMs = config.hrmHoldThresholdMs ?? 150
  manipulators.push(
    bareKeyMap(key)
      .to(streak)
      .to(toSetVar(heldVar, 1) as ToEvent)
      .to(toSetVar(readyVar, 0) as ToEvent) // NOT ready yet
      .to(toSetVar(outputtedVar, 0) as ToEvent)
      .toIfHeldDown(toSetVar(readyVar, 1) as ToEvent) // Ready after hold threshold
      .parameters({ 'basic.to_if_held_down_threshold_milliseconds': holdThresholdMs })
      .toAfterKeyUp([
        toSetVar(heldVar, 0),
        toSetVar(readyVar, 0),
        toSetVar(outputtedVar, 0),
      ] as ToEvent[])
      .toIfAlone(key as any)
  )

  return manipulators
}

/**
 * Add all HRM behaviors for a key.
 * For HRM source keys: full pipeline (activation, rollover, source)
 * For regular keys: just target behavior (activation + rollover)
 */
export const addHrm: Transformer = (ctx) => {
  const { key, config } = ctx

  if (isHrmKey(key, config)) {
    // HRM source keys: full pipeline
    return addHrmSource(ctx)
  }

  // Regular keys: just target behavior
  return [
    ...addHrmActivation(ctx),
    ...addHrmRollover(ctx),
  ]
}

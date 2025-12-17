import { ifVar, toSetVar, type ToEvent } from 'karabiner.ts'
import type { PipelineContext, Transformer, Binding } from '../types'
import { streakUpdates } from '../utils/streak'
import { bareKeyMap } from './default'

/**
 * Add layer triggers and bindings.
 * Layer triggers set/unset layer variables.
 * Layer bindings output mapped keys when layer is active.
 */
export const addLayers: Transformer = (ctx) => {
  const { key, config, kb } = ctx
  const layers = config.layers ?? {}
  const hrm = config.hrm ?? {}
  const hrmKeys = Object.keys(hrm)
  const windowMs = config.streakWindowMs ?? 150
  const streak = streakUpdates(windowMs)
  const manipulators: ReturnType<typeof bareKeyMap>[] = []

  // Check if this key is a layer trigger
  const triggerLayer = Object.entries(layers).find(([, layer]) => layer.trigger === key)

  if (triggerLayer) {
    const [layerName, layer] = triggerLayer
    const activeVar = `${layerName}Active`
    const parentConditions = layer.parent ? [ifVar(`${layer.parent}Active`, 1)] : []
    const layerHoldMs = config.layerHoldThresholdMs ?? 120

    // First: Handle HRM rollover - if HRM key is held but NOT ready (still typing),
    // output the letter before activating the layer
    // If Ready=1 (deliberate hold), skip this and let normal layer activation happen
    for (const hrmKey of hrmKeys) {
      const heldVar = `${hrmKey}Held`
      const readyVar = `${hrmKey}HeldReady`
      const outputtedVar = `${hrmKey}Outputted`

      manipulators.push(
        bareKeyMap(key)
          .condition(...parentConditions, ifVar(heldVar, 1), ifVar(readyVar, 0), ifVar(outputtedVar, 0))
          .to(streak)
          .to(hrmKey as any) // Output the held HRM key first
          .to(toSetVar(outputtedVar, 1) as ToEvent)
          .to(toSetVar(activeVar, 0) as ToEvent) // NOT active yet
          .toIfHeldDown(toSetVar(activeVar, 1) as ToEvent) // Active after threshold
          .parameters({ 'basic.to_if_held_down_threshold_milliseconds': layerHoldMs })
          .toAfterKeyUp(toSetVar(activeVar, 0) as ToEvent)
          .toIfAlone(key as any)
      )
    }

    // Then: Normal layer trigger (no HRM held)
    manipulators.push(
      bareKeyMap(key)
        .condition(...parentConditions)
        .to(streak)
        .to(toSetVar(activeVar, 0) as ToEvent) // NOT active yet
        .toIfHeldDown(toSetVar(activeVar, 1) as ToEvent) // Active after threshold
        .parameters({ 'basic.to_if_held_down_threshold_milliseconds': layerHoldMs })
        .toAfterKeyUp(toSetVar(activeVar, 0) as ToEvent)
        .toIfAlone(key as any)
    )

    return manipulators
  }

  // Not a trigger - check for layer bindings
  // Sort layers: children before parents (more specific first)
  const sortedLayers = Object.entries(layers).sort(([, a], [, b]) => {
    if (a.parent && !b.parent) return -1
    if (!a.parent && b.parent) return 1
    return 0
  })

  for (const [layerName, layer] of sortedLayers) {
    let binding: Binding | undefined

    if (key in layer.bindings) {
      binding = layer.bindings[key]
    } else if (layer.inherit && layer.parent) {
      // Check inherited bindings from parent
      const parentLayer = layers[layer.parent]
      if (parentLayer && key in parentLayer.bindings) {
        binding = parentLayer.bindings[key]
      }
    }

    if (binding) {
      const [outputKey, outputMod] = Array.isArray(binding)
        ? binding
        : [binding, undefined]

      // Build condition chain: this layer + ancestors if no handoff
      const conditions = [ifVar(`${layerName}Active`, 1)]
      if (layer.handoff === false && layer.parent) {
        conditions.push(ifVar(`${layer.parent}Active`, 1))
      }

      manipulators.push(
        bareKeyMap(key)
          .condition(...conditions)
          .to(streak)
          .to(outputKey as any, outputMod as any)
      )
    }
  }

  // Swallow undefined keys when layer is active (no passthrough)
  for (const [layerName, layer] of sortedLayers) {
    if (!(key in layer.bindings) && key !== layer.trigger) {
      manipulators.push(
        bareKeyMap(key)
          .condition(ifVar(`${layerName}Active`, 1))
          .to('vk_none' as any)
      )
    }
  }

  return manipulators
}

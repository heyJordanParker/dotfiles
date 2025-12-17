import { rule } from 'karabiner.ts'
import type { KeyboardConfig } from './types'
import { parseLayout } from './layout'
import { addLayers } from './pipeline/layers'
import { addHrm } from './pipeline/hrm'
import { addDefault } from './pipeline/default'

// Re-export types for index.ts
export type { KeyboardConfig, LayerDef } from './types'

// =============================================================================
// Main Generator
// =============================================================================

export function generateKeyboardRules(config: KeyboardConfig) {
  const kb = parseLayout(config.layout)
  const layers = config.layers ?? {}

  // Layer triggers may not be in layout (e.g., spacebar)
  const layerTriggers = new Set(Object.values(layers).map((l) => l.trigger))
  const allKeys = new Set([...kb.all, ...layerTriggers])

  const allManipulators: ReturnType<typeof import('karabiner.ts').map>[] = []

  // Pipeline: layers → hrm → default
  for (const key of allKeys) {
    const ctx = { key, config, kb }

    // 1. Layer triggers + bindings (highest priority)
    allManipulators.push(...addLayers(ctx))

    // 2. HRM behaviors (activation, rollover, source)
    allManipulators.push(...addHrm(ctx))

    // 3. Default (bare key passthrough) - skip for layer triggers
    if (!layerTriggers.has(key)) {
      allManipulators.push(...addDefault(ctx))
    }
  }

  return [rule('Keyboard').manipulators(allManipulators as any)]
}

import type { ToEvent } from 'karabiner.ts'

// =============================================================================
// Config Types
// =============================================================================

export type ModifierKey = '⌃' | '⌘' | '⇧' | '⌥'
export type Binding = string | [string, string] // key or [key, modifier]

export interface LayerDef {
  trigger: string
  parent?: string
  handoff?: boolean // default true
  inherit?: boolean // default false
  bindings: Record<string, Binding>
}

export interface KeyboardConfig {
  layout: string[]
  hrm?: Record<string, ModifierKey>
  layers?: Record<string, LayerDef>
  streakWindowMs?: number
  hrmHoldThresholdMs?: number // How long to hold before HRM activates (default 120)
  layerHoldThresholdMs?: number // How long to hold before layer activates (default 120)
}

export interface ParsedKeyboard {
  left: string[]
  right: string[]
  all: string[]
}

// =============================================================================
// Pipeline Types
// =============================================================================

export interface PipelineContext {
  key: string
  config: KeyboardConfig
  kb: ParsedKeyboard
}

export type Transformer = (ctx: PipelineContext) => ReturnType<typeof import('karabiner.ts').map>[]

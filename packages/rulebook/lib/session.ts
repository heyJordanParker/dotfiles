import { readFileSync, statSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import type { Call } from '@cmodjs/rulebook'

export type Mode = 'orchestrate' | 'build'

export type Stage = 'propose' | 'execute'

const MODES: readonly string[] = ['orchestrate', 'build']

const STAGES: readonly string[] = ['propose', 'execute']

const SESSION_ID = /^[A-Za-z0-9_][A-Za-z0-9_-]*$/

const KEY = /^([A-Za-z_][\w-]*):\s?(.*)$/

const WRITE_TOOLS: readonly string[] = ['Write', 'Edit', 'MultiEdit', 'NotebookEdit']

export function ownerSession(call: Call): string {
  return process.env.CLAUDE_CODE_SESSION_ID || call.session || ''
}

export function isDispatched(call: Call): boolean {
  return call.agentId !== undefined
}

export function sessionState(session: string): Readonly<Record<string, unknown>> {
  if (!SESSION_ID.test(session)) return {}
  const root = join(process.env.CLAUDE_DATA_ROOT || join(homedir(), '.claude'), 'sessions')
  const folder = [join(root, session), ...matches(root, `*/subagents/${session}`).sort()].find(isFolder)
  if (folder === undefined) return {}
  try {
    const state: unknown = JSON.parse(readFileSync(join(folder, 'state.json'), 'utf8'))
    return typeof state === 'object' && state !== null && !Array.isArray(state) ? (state as Record<string, unknown>) : {}
  } catch {
    return {}
  }
}

export function agentName(call: Call): string {
  const named = call.agentType ?? ''
  return named === '' ? '' : startedAs(call, named) || named
}

export function definitionPath(name: string): string {
  if (name === '') return ''
  const project = process.env.CLAUDE_PROJECT_DIR
  const local = project ? join(project, '.claude', 'agents', `${name}.md`) : ''
  if (local !== '' && isFile(local)) return local
  return join(configFolder(), 'agents', `${name}.md`)
}

export function isReadonly(path: string): boolean {
  return declares(path, 'readonly', 'true', true)
}

export function allowsSsh(path: string): boolean {
  return declares(path, 'ssh', 'enabled', false)
}

export function declaresNoWriteTool(path: string): boolean {
  let tools: string | undefined
  try {
    tools = declaration(path, 'tools')
  } catch {
    return false
  }
  if (tools === undefined) return false
  const named = tools.replace(/^\[|\]$/g, '').split(',').map((tool) => tool.trim()).filter((tool) => tool !== '')
  return named.length > 0 && !named.some((tool) => WRITE_TOOLS.includes(tool))
}

export function declaredMode(call: Call): Mode {
  const path = definitionPath(agentName(call))
  if (path === '') return 'build'
  try {
    const mode = declaration(path, 'mode')
    return mode !== undefined && MODES.includes(mode) ? (mode as Mode) : 'build'
  } catch {
    return 'build'
  }
}

export function mode(call: Call): Mode {
  const declared = declaredMode(call)
  const session = ownerSession(call)
  if (session === '') return declared
  const typed = sessionState(session).mode
  return typeof typed === 'string' && MODES.includes(typed) ? (typed as Mode) : declared
}

export function maySpawn(call: Call): boolean {
  if (isDispatched(call)) return declaredMode(call) === 'orchestrate'
  if (agentName(call) !== '' || sessionState(ownerSession(call)).mode_typed) return mode(call) === 'orchestrate'
  return true
}

export function stage(call: Call): Stage {
  const value = sessionState(ownerSession(call)).state
  if (typeof value === 'string' && STAGES.includes(value)) return value as Stage
  return isDispatched(call) ? 'execute' : 'propose'
}

function startedAs(call: Call, named: string): string {
  if (isFile(definitionPath(named))) return ''
  for (const record of startRecords(call, named)) {
    try {
      const started: unknown = JSON.parse(readFileSync(record, 'utf8')).customAgentType
      if (typeof started === 'string' && started !== '') return started
    } catch {}
  }
  return ''
}

function startRecords(call: Call, named: string): readonly string[] {
  const name = call.agentId === undefined ? `agent-a${named}-*.meta.json` : `agent-${call.agentId}.meta.json`
  if (call.transcriptPath?.endsWith('.jsonl') === true) return matches(join(call.transcriptPath.slice(0, -'.jsonl'.length), 'subagents'), name)
  if (call.session === undefined || !SESSION_ID.test(call.session)) return []
  return matches(join(configFolder(), 'projects'), `*/${call.session}/subagents/${name}`)
}

function matches(folder: string, glob: string): string[] {
  return isFolder(folder) ? [...new Bun.Glob(glob).scanSync({ cwd: folder, absolute: true, onlyFiles: false })] : []
}

function declares(path: string, key: string, value: string, unreadable: boolean): boolean {
  let text: string | undefined
  try {
    text = definitionText(path)
  } catch {
    return unreadable
  }
  return text !== undefined && declared(text, key)?.toLowerCase() === value
}

function declaration(path: string, key: string): string | undefined {
  const text = definitionText(path)
  return text === undefined ? undefined : declared(text, key)
}

function definitionText(path: string): string | undefined {
  try {
    return readFileSync(path, 'utf8')
  } catch (error) {
    if (error instanceof Error && 'code' in error && (error.code === 'ENOENT' || error.code === 'ENOTDIR')) return undefined
    throw error
  }
}

function declared(text: string, key: string): string | undefined {
  const lines = text.split(/\r?\n/)
  if (lines[0]?.trim() !== '---') return undefined
  for (const line of lines.slice(1)) {
    if (line.trim() === '---') return undefined
    const match = KEY.exec(line)
    if (match?.[1] === key) return valueOf(match[2] ?? '')
  }
  return undefined
}

function valueOf(rest: string): string {
  const value = rest.trim()
  if (value.length >= 2 && (value[0] === '"' || value[0] === "'")) {
    const close = value.indexOf(value[0], 1)
    if (close > 0) return value.slice(1, close).trim()
  }
  return (value.split('#', 1)[0] ?? '').trim()
}

function configFolder(): string {
  return process.env.CLAUDE_CONFIG_DIR || join(homedir(), '.claude')
}

function isFile(path: string): boolean {
  return statSync(path, { throwIfNoEntry: false })?.isFile() === true
}

function isFolder(path: string): boolean {
  return statSync(path, { throwIfNoEntry: false })?.isDirectory() === true
}

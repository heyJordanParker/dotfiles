import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { resolve } from 'node:path'
import type { Call, Command, Context } from '@cmodjs/rulebook'

const INFO_FLAGS: ReadonlySet<string> = new Set(['--version', '-V', '-v', '--help', '-h'])

const MANAGEMENT: Readonly<Record<string, ReadonlySet<string>>> = {
  claude: new Set(['mcp', 'plugin', 'plugins', 'auth', 'doctor', 'install', 'update', 'upgrade', 'setup-token', 'auto-mode', 'import', 'purge', 'logs', 'stop', 'kill', 'rm']),
  codex: new Set(['mcp', 'plugin', 'login', 'logout', 'completion', 'update', 'doctor', 'features', 'help', 'apply', 'archive', 'unarchive', 'delete']),
}

const SCRIPT_MODEL = /\bmodel\s*:\s*['"`]([^'"`]*)/gi

export function launchesAgent(call: Call): boolean {
  return ['Agent', 'Workflow', 'RemoteTrigger'].includes(call.tool) || call.tool.endsWith('spawn_agent')
}

export function wouldSpawn(context: Context): boolean {
  const call = context.call
  if (call === undefined) return false
  if (launchesAgent(call)) return true
  return context.path === '@command' && context.commands.some(startsSession)
}

export function choosesFable(context: Context): boolean {
  const call = context.call
  if (call === undefined) return false
  if (call.tool === 'Workflow') return [...workflowScript(call).matchAll(SCRIPT_MODEL)].some(([, model]) => isFable(model))
  if (launchesAgent(call)) return isFable(call.input.model)
  return claudeLaunches(context).some((args) => modelsOf(args).some(isFable))
}

function startsSession({ program, args }: Command): boolean {
  const management = MANAGEMENT[program]
  if (management === undefined) return false
  if (args[0] !== undefined && management.has(args[0])) return false
  return !(args.length === 1 && INFO_FLAGS.has(args[0] ?? ''))
}

function claudeLaunches(context: Context): readonly (readonly string[])[] {
  return context.path === '@command' ? context.commands.filter(({ program }) => program === 'claude').map(({ args }) => args) : []
}

function modelsOf(args: readonly string[]): readonly string[] {
  return args.flatMap((arg, index) => (arg === '--model' && index + 1 < args.length ? [args[index + 1] ?? ''] : arg.startsWith('--model=') ? [arg.slice('--model='.length)] : []))
}

function workflowScript(call: Call): string {
  const path = call.input.scriptPath
  if (typeof path !== 'string' || path === '') return typeof call.input.script === 'string' ? call.input.script : ''
  try {
    return readFileSync(resolve(call.cwd, path.replace(/^~(?=\/|$)/, homedir())), 'utf8')
  } catch {
    return ''
  }
}

function isFable(model: unknown): boolean {
  return String(model ?? '').toLowerCase().includes('fable')
}

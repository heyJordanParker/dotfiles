import { realpathSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join, normalize, resolve } from 'node:path'
import type { Command, Context } from '@cmodjs/rulebook'
import { isInTmp } from './shell.ts'

const HOME = homedir()

const ALLOWED_PREFIXES: readonly string[] = [
  normalize(join(realpathSync(import.meta.dir), '..', '..', '..')),
  `${HOME}/Developer`,
  `${HOME}/Downloads`,
  `${HOME}/Desktop`,
  `${HOME}/conductor`,
  `${HOME}/.claude`,
]

const ZSH_RUNNERS: ReadonlySet<string> = new Set(['exec', 'time', 'zsh'])

const NO_RCS: ReadonlySet<string> = new Set(['-f', '--no-rcs'])

const HOME_PREFIX = /^(~|\$HOME|\$\{HOME\})(?=\/|$)/

const UNPROVABLE = /[$`]|^~/

const GLOB = /[*?[]/

export function skipsTheTrash({ commands }: Context): boolean {
  return commands.some((command) => (command.program === 'find' ? command.args.includes('-delete') : command.program === 'rm' && (command.argv[0] !== 'rm' || !runsInZsh(command, commands))))
}

export function deletesOutsideAllowedFolders(context: Context): boolean {
  const cwd = context.call?.cwd ?? context.root
  return context.commands.some((command) => command.program === 'rm' && operandsOf(command.args).some((operand) => !isAllowed(judgedPath(operand, command.folder, cwd))))
}

function runsInZsh({ runner }: Command, commands: readonly Command[]): boolean {
  if (runner === undefined) return true
  const command = commands[runner]
  return command !== undefined && ZSH_RUNNERS.has(command.program) && !command.args.some((arg) => NO_RCS.has(arg)) && runsInZsh(command, commands)
}

function operandsOf(args: readonly string[]): readonly string[] {
  const end = args.indexOf('--')
  const options = end < 0 ? args : args.slice(0, end)
  return [...options.filter((arg) => !arg.startsWith('-')), ...(end < 0 ? [] : args.slice(end + 1))]
}

function judgedPath(operand: string, folder: string, cwd: string): string | undefined {
  const target = expanded(operand)
  const where = expanded(folder)
  if (UNPROVABLE.test(target) || UNPROVABLE.test(where)) return undefined
  const glob = target.search(GLOB)
  if (glob < 0) return resolve(cwd, where, target)
  const literal = target.slice(0, glob)
  if (target.slice(glob).split('/').includes('..')) return undefined
  const path = resolve(cwd, where, literal)
  return literal === '' || literal.endsWith('/') ? path : dirname(path)
}

function expanded(path: string): string {
  return path.replace(HOME_PREFIX, HOME)
}

function isAllowed(path: string | undefined): boolean {
  return path !== undefined && (isInTmp(path) || ALLOWED_PREFIXES.some((prefix) => `${path}/`.startsWith(`${prefix}/`)))
}

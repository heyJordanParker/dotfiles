import { realpathSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, normalize } from 'node:path'
import { isInTmp } from './shell.ts'

export type DeleteRefusal = 'unresolved' | 'outside' | ''

const HOME = homedir()

const ALLOWED_PREFIXES: readonly string[] = [
  normalize(join(realpathSync(import.meta.dir), '..', '..', '..')),
  `${HOME}/Developer`,
  `${HOME}/Downloads`,
  `${HOME}/Desktop`,
  `${HOME}/conductor`,
  `${HOME}/.claude`,
]

const RM = /(^|[;&|\s$(`])\\?(\S*\/)?rm(\s|$)/

const PIPED_RM = /\|.*rm(\s|$)/

const SUBSTITUTED_RM = /(\$\(|`)\\?(\S*\/)?rm\s/

const EXPANSION = /[$`]/

const GLOB = /[*?[]/

export function deleteRefusal(command: string, cwd: string): DeleteRefusal {
  if (!RM.test(command)) return ''
  if (PIPED_RM.test(command) || SUBSTITUTED_RM.test(command)) return 'unresolved'
  if (!/rm\s+/.test(command)) return ''
  const operands = command.replace(/.*rm\s+/g, '').split(/\s+/).filter((word) => word !== '' && !word.startsWith('-'))
  if (operands.some((word) => EXPANSION.test(word))) return 'unresolved'
  if (operands.some((word) => GLOB.test(word))) return isAllowed(cwd) ? '' : 'outside'
  return operands.every((word) => isAllowed(normalize(word.startsWith('~') ? `${HOME}${word.slice(1)}` : word.startsWith('/') ? word : `${cwd}/${word}`))) ? '' : 'outside'
}

function isAllowed(path: string): boolean {
  return isInTmp(path) || ALLOWED_PREFIXES.some((prefix) => path.startsWith(prefix))
}

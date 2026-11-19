import { absolutePath, isInTmp } from './shell.ts'

const DEVICES: ReadonlySet<string> = new Set(['/dev/null', '/dev/stdout', '/dev/stderr', '/dev/tty'])

const PLAN_FOLDERS: readonly string[] = ['/docs/shaping/', '/docs/plans/', '/docs/agents/', '/.claude/shaping/', '/.claude/plans/']

export const WRITE_TOOLS: ReadonlySet<string> = new Set(['Write', 'Edit', 'MultiEdit', 'NotebookEdit', 'apply_patch'])

export function isWritableWhileProposing(target: string, cwd: string): boolean {
  if (DEVICES.has(target.trim().replace(/^["']+|["']+$/g, ''))) return true
  const path = absolutePath(target.trim(), cwd)
  return PLAN_FOLDERS.some((folder) => `${path}/`.includes(folder)) || isInTmp(path)
}

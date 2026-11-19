import { resolve, sep } from 'node:path'
import { defineRule } from '@cmodjs/rulebook'
import { ownerSession, sessionState } from '../../lib/session.ts'
import { shellOf } from '../../lib/shell.ts'

const FREE_COMMIT_ROOTS: readonly string[] = ['/Users/jordan/Developer/php-sharp', '/Users/jordan/Developer/mago-sharp', '/Users/jordan/Developer/sharp-worktrees']

export default defineRule({
  description: "Don't commit until the Architect authorizes it.",
  message: 'The Architect authorizes a commit by typing /commit, for that turn only.',
  files: ['@command'],
  create(context) {
    const call = context.call
    if (call === undefined) return {}
    const isAuthorized = (): boolean => {
      const session = ownerSession(call)
      return session === '' || sessionState(session).commit_requested === true
    }
    const { sourceCode } = context
    if (shellOf(context)?.commands.length === 0 && sourceCode.text.includes('commit') && !isAuthorized()) context.report({ node: sourceCode.ast })
    return {
      Command(command) {
        if (command.program !== 'git' || command.args[0] !== 'commit') return
        const repository = resolve(call.cwd, command.folder)
        const isFree = FREE_COMMIT_ROOTS.some((root) => repository === root || repository.startsWith(`${root}${sep}`))
        if (!isFree && !isAuthorized()) context.report({ node: command })
      },
    }
  },
})

import { defineRule } from '@cmodjs/rulebook'
import { shellOf } from '../../lib/shell.ts'

const READS: ReadonlySet<string | undefined> = new Set(['list', 'show'])

export default defineRule({
  description: "Don't run git stash.",
  message: "A stash hides other agents' uncommitted work in this worktree. Only git stash list and git stash show run.",
  files: ['@command'],
  create(context) {
    const { sourceCode } = context
    if (shellOf(context)?.commands.length === 0 && /git\s+stash/.test(sourceCode.text)) context.report({ node: sourceCode.ast })
    return {
      Command(command) {
        if (command.program === 'git' && command.args[0] === 'stash' && !READS.has(command.args[1])) context.report({ node: command })
      },
    }
  },
})

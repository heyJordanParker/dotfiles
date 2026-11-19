import { defineRule } from '@cmodjs/rulebook'
import { shellOf } from '../../lib/shell.ts'

export default defineRule({
  description: "Don't run git clean.",
  message: "It deletes every untracked file in the tree, other agents' new files included. Remove a file you created with rm <path>.",
  files: ['@command'],
  create(context) {
    const { sourceCode } = context
    if (shellOf(context)?.commands.length === 0 && /git\s+clean/.test(sourceCode.text)) context.report({ node: sourceCode.ast })
    return {
      Command(command) {
        if (command.program === 'git' && command.args[0] === 'clean') context.report({ node: command })
      },
    }
  },
})

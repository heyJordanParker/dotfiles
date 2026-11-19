import { defineRule } from '@cmodjs/rulebook'
import { shellOf } from '../../lib/shell.ts'

export default defineRule({
  description: "Don't run git restore.",
  message: 'Undo specific lines with the Edit tool. To prove a failure is pre-existing, report the command and its red output instead.',
  files: ['@command'],
  create(context) {
    const { sourceCode } = context
    if (shellOf(context)?.commands.length === 0 && /git\s+restore/.test(sourceCode.text)) context.report({ node: sourceCode.ast })
    return {
      Command(command) {
        if (command.program === 'git' && command.args[0] === 'restore') context.report({ node: command })
      },
    }
  },
})

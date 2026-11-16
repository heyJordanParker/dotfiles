import { defineRule } from '@cmodjs/rulebook'

export default defineRule({
  description: "Don't run git restore.",
  message: 'Undo specific lines with the Edit tool. To prove a failure is pre-existing, report the command and its red output instead.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (context.commands.length === 0 && /git\s+restore/.test(context.text)) context.report({ node: document })
      },
      Command(command) {
        if (command.program === 'git' && command.args[0] === 'restore') context.report({ node: command })
      },
    }
  },
})

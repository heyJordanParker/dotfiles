import { defineRule } from '@cmodjs/rulebook'

export default defineRule({
  description: "Don't run git clean.",
  message: "It deletes every untracked file in the tree, other agents' new files included. Remove a file you created with rm <path>.",
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (context.commands.length === 0 && /git\s+clean/.test(context.text)) context.report({ node: document })
      },
      Command(command) {
        if (command.program === 'git' && command.args[0] === 'clean') context.report({ node: command })
      },
    }
  },
})

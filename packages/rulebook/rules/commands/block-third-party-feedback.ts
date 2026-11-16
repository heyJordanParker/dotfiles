import { defineRule } from '@cmodjs/rulebook'

export default defineRule({
  description: "Don't send 3rd party software feedback.",
  files: ['@command'],
  create(context) {
    return {
      Command(command) {
        if (command.program === 'herdr-app' && command.args[0] === 'feedback') context.report({ node: command })
      },
    }
  },
})

import { defineRule } from '@cmodjs/rulebook'
import { typesOrchestrate } from '../../lib/spawn.ts'

export default defineRule({
  description: "Don't type /orchestrate into a session you launch.",
  message: 'Only the Architect makes a session orchestrate. Launch it without /orchestrate.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (typesOrchestrate(context)) context.report({ node: document })
      },
    }
  },
})

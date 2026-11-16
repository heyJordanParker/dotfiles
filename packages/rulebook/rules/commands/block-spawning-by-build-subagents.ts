import { defineRule } from '@cmodjs/rulebook'
import { isDispatched, maySpawn } from '../../lib/session.ts'
import { wouldSpawn } from '../../lib/spawn.ts'

export default defineRule({
  description: "Don't start another agent from a Subagent whose definition declares build.",
  message: 'Do the work yourself, or return what needs another agent to your dispatcher.',
  files: ['@command', '@spawn'],
  create(context) {
    const call = context.call
    if (call === undefined || !isDispatched(call) || maySpawn(call)) return {}
    return {
      Document(document) {
        if (wouldSpawn(context)) context.report({ node: document })
      },
    }
  },
})

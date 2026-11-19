import { defineRule } from '@cmodjs/rulebook'
import { choosesFable } from '../../lib/spawn.ts'

export default defineRule({
  description: "Don't launch an agent on Fable.",
  message: 'Only the Architect uses Fable. Drop the model override, or name another model.',
  files: ['@command', '@spawn'],
  create(context) {
    if (choosesFable(context)) context.report({ node: context.sourceCode.ast })
    return {}
  },
})

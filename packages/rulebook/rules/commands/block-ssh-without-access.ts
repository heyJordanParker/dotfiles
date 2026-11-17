import { defineRule } from '@cmodjs/rulebook'
import { agentName, allowsSsh, definitionPath, isDispatched } from '../../lib/session.ts'
import { REMOTE } from '../../lib/shell.ts'

export default defineRule({
  description: "Don't reach another machine as an agent that does not declare ssh: enabled.",
  message: 'Your definition does not declare ssh: enabled. Report what you need from that machine, and the caller runs it.',
  files: ['@command'],
  create(context) {
    const { call } = context
    const definition = call === undefined || !isDispatched(call) ? '' : definitionPath(agentName(call))
    if (definition === '' || allowsSsh(definition)) return {}
    return {
      Command(command) {
        if (REMOTE.has(command.program)) context.report({ node: command })
      },
    }
  },
})

import { defineRule } from '@cmodjs/rulebook'
import { agentName, declaresNoWriteTool, definitionPath, isDispatched, isReadonly } from '../../lib/session.ts'
import { readonlyRefusal, shellOf } from '../../lib/shell.ts'

const WRITE_TOOLS: ReadonlySet<string> = new Set(['Write', 'Edit', 'MultiEdit', 'NotebookEdit', 'apply_patch'])

export default defineRule({
  description: "Don't change anything as an agent that declares readonly: true.",
  message: 'Your definition declares readonly: true, so you read, trace, and report. The caller makes the change from your report, on both harnesses. If reading needs a command this refuses, name it in your report.',
  files: ['**', '@command'],
  create(context) {
    const call = context.call
    const definition = call === undefined || !isDispatched(call) ? '' : definitionPath(agentName(call))
    if (call === undefined || definition === '') return {}
    if (WRITE_TOOLS.has(call.tool)) {
      if (isReadonly(definition) || declaresNoWriteTool(definition)) context.report({ node: context.sourceCode.ast })
      return {}
    }
    if (context.filename !== '@command' || !isReadonly(definition)) return {}
    if ((shellOf(context)?.writes.length ?? 0) > 0) context.report({ node: context.sourceCode.ast })
    return {
      Command(command) {
        if (readonlyRefusal(command.program, command.args) !== '') context.report({ node: command })
      },
    }
  },
})

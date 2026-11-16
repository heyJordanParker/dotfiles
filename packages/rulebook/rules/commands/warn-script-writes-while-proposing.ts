import { defineRule } from '@cmodjs/rulebook'
import { isWritableWhileProposing, WRITE_TOOLS, writtenFile } from '../../lib/proposal.ts'
import { ownerSession, stage } from '../../lib/session.ts'

const SCRIPT = /\.(py|sh|bash|zsh|js|mjs|cjs|ts|rb|pl)$/

export default defineRule({
  description: 'Run a script written while proposing only once the proposal is approved.',
  message: 'Running it with python, node, bash, or another runtime is refused until the Architect approves the proposal.',
  files: ['**'],
  severity: 'warn',
  create(context) {
    const call = context.call
    if (call === undefined || !WRITE_TOOLS.has(call.tool) || ownerSession(call) === '' || stage(call) !== 'propose') return {}
    return {
      Document(document) {
        const file = writtenFile(context)
        if (SCRIPT.test(file) && isWritableWhileProposing(file, call.cwd)) context.report({ node: document })
      },
    }
  },
})

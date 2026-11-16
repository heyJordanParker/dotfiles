import { defineRule } from '@cmodjs/rulebook'
import { writtenFile } from '../../lib/proposal.ts'

const STATE_FILE = /\/sessions\/[^ ]*state\.json/

const TOUCHES_STATE_FILE = /(echo|printf|cat|rm|mv|cp|jq|tee|chmod|chown|ls|stat|head|tail|sed|awk)\b.*\/sessions\/[^ ]*state\.json|[>].*\/sessions\/[^ ]*state\.json/

export default defineRule({
  description: "Don't touch a session's state file.",
  message: 'Session hooks own it. The Architect changes the mode by typing /orchestrate or /build, and the stage by typing /propose or /execute.',
  files: ['**/state.json', '@command'],
  create(context) {
    if (context.call === undefined || process.env.CLAUDE_SESSION_HOOK === 'true') return {}
    return {
      Document(document) {
        const touches = context.path === '@command' ? TOUCHES_STATE_FILE.test(context.text) : STATE_FILE.test(writtenFile(context))
        if (touches) context.report({ node: document })
      },
    }
  },
})

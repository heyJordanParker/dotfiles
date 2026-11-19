import { defineRule } from '@cmodjs/rulebook'

const STATE_FILE = /\/sessions\/[^ ]*state\.json/

const TOUCHES_STATE_FILE = /(echo|printf|cat|rm|mv|cp|jq|tee|chmod|chown|ls|stat|head|tail|sed|awk)\b.*\/sessions\/[^ ]*state\.json|[>].*\/sessions\/[^ ]*state\.json/

export default defineRule({
  description: "Don't touch a session's state file.",
  message: 'Session hooks own it. The Architect changes the mode by typing /orchestrate or /build, and the stage by typing /propose or /execute.',
  files: ['**/state.json', '@command'],
  create(context) {
    if (context.call === undefined || process.env.CLAUDE_SESSION_HOOK === 'true') return {}
    const { filename, sourceCode } = context
    const touches = filename === '@command' ? TOUCHES_STATE_FILE.test(sourceCode.text) : STATE_FILE.test(filename)
    if (touches) context.report({ node: sourceCode.ast })
    return {}
  },
})

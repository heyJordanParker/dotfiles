import { defineRule } from '@cmodjs/rulebook'
import { findInProse } from '../lib/prose.ts'

const OFFERS = /\b(if you want|if you'd like|do you want|want me to|would you like|should I|let me know)\b/i

export default defineRule({
  description: "Don't offer the Architect more work.",
  message: 'Do the reversible work, or propose it as a Solution.',
  files: ['@reply', '@inbox/**'],
  create(context) {
    for (const start of findInProse(context.sourceCode, OFFERS)) context.report({ loc: { start } })
    return {}
  },
})

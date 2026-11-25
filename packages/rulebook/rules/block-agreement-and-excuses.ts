import { defineRule } from '@cmodjs/rulebook'
import { findInProse } from '../lib/prose.ts'

const AGREEMENT = /\b(you're right|you are right|you were right|I was wrong|my mistake|I should have)\b|^\W*understood\b/i

export default defineRule({
  description: "Don't agree, apologize, or excuse yourself in a reply.",
  message: 'Start with the result.',
  files: ['@reply', '@inbox/**'],
  create(context) {
    for (const start of findInProse(context.sourceCode, AGREEMENT)) context.report({ loc: { start } })
    return {}
  },
})

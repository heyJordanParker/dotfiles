import { defineRule } from '@cmodjs/rulebook'
import { findInProse } from '../lib/prose.ts'

const HEDGES = /\b(probably|perhaps)\b/i

export default defineRule({
  description: "Don't hedge a claim you can check.",
  message: 'Check the claim, then state it.',
  files: ['@reply', '@inbox/**'],
  create(context) {
    for (const start of findInProse(context.sourceCode, HEDGES)) context.report({ loc: { start } })
    return {}
  },
})

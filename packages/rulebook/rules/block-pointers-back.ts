import { defineRule } from '@cmodjs/rulebook'
import { findInProse } from '../lib/prose.ts'

const POINTERS = /\b(my (earlier|previous|last)|(the|my|your) last reply|I said|(as|see) above|as (mentioned|noted|shown|described) (above|earlier|before)|the (tree|table|diagram|list|drawing|reply|message|section|proposal|plan) above)\b/i

export default defineRule({
  description: "Don't point the Architect back to an earlier reply.",
  message: 'Say it again in this reply. The Architect reads it alone.',
  files: ['@reply', '@inbox/**'],
  create(context) {
    for (const start of findInProse(context.sourceCode, POINTERS)) context.report({ loc: { start } })
    return {}
  },
})

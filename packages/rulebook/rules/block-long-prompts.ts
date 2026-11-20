import { defineRule } from '@cmodjs/rulebook'

const LIMIT = 10_000

export default defineRule({
  description: 'Keep a Prompt under 10,000 characters, so it survives compaction and renders in hooks.',
  create(context) {
    const { length } = context.sourceCode.text
    if (length >= LIMIT && length > (context.previous?.text.length ?? 0)) context.report({ node: context.sourceCode.ast })
    return {}
  },
})

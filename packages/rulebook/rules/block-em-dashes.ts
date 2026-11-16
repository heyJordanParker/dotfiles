import { defineRule } from '@cmodjs/rulebook'

const EM_DASH = String.fromCharCode(0x2014)

export default defineRule({
  description: "Don't write em dashes.",
  message: 'An em dash joins two ideas in one sentence. Split it into two sentences.',
  files: ['**', '@reply', '@inbox/**'],
  create(context) {
    return {
      Line(line) {
        if (!line.isInCodeBlock && line.text.includes(EM_DASH)) context.report({ node: line })
      },
    }
  },
})

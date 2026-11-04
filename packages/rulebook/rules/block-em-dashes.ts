import { defineRule } from '@cmodjs/rulebook'

const FENCE = /^\s*(```|~~~)/

export default defineRule({
  description: "Don't write em dashes.",
  paths: ['**', '@reply'],
  check: ({ lines }) => {
    let isCode = false
    return lines.flatMap((text, index) => {
      if (FENCE.test(text)) isCode = !isCode
      if (isCode || !text.includes('\u2014')) return []
      return [{ line: index + 1, message: 'An em dash joins two ideas in one sentence. Split it into two sentences.' }]
    })
  },
})

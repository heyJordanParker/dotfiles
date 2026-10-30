import { defineRule } from '@cmodjs/plint'

const FENCE = /^\s*(```|~~~)/

export default defineRule({
  level: 'error',
  check: ({ lines }) => {
    let isCode = false
    return lines.flatMap((text, index) => {
      if (FENCE.test(text)) isCode = !isCode
      if (isCode || !text.includes('—')) return []
      return [{ line: index + 1, message: 'An em dash joins two ideas in one sentence. Split it into two sentences.' }]
    })
  },
})

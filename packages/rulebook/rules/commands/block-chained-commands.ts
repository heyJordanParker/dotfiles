import { defineRule } from '@cmodjs/rulebook'
import { isSeparator, localLines, tokens } from '../../lib/shell.ts'

export default defineRule({
  description: 'Run one command per shell call.',
  message: 'Send each command as its own call, and read its result before you choose the next one.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        const joins = localLines(context).some((line) => (tokens(line) ?? []).some(({ text, isOperator }) => isOperator && isSeparator(text) && text !== '|' && !/^[()]+$/.test(text)))
        if (joins) context.report({ node: document })
      },
    }
  },
})

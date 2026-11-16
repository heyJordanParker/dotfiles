import { defineRule } from '@cmodjs/rulebook'
import { headAndArgs, localLines, nameOf, readonlyRefusal, segments } from '../../lib/shell.ts'

export default defineRule({
  description: "Don't pipe into a command that changes anything.",
  message: 'A pipeline that ends in a change is a program. Run the read, then make the change in its own call.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        const changes = localLines(context).some((line) => (segments(line) ?? []).some(({ words, piped }) => piped && nameOf(words[0] ?? '') !== 'xargs' && readonlyRefusal(...headAndArgs(words)) !== ''))
        if (changes) context.report({ node: document })
      },
    }
  },
})

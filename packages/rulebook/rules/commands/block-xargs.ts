import { defineRule } from '@cmodjs/rulebook'
import { localLines, nameOf, segments } from '../../lib/shell.ts'

export default defineRule({
  description: "Don't apply a command to a batch with xargs.",
  message: 'Act on one target per call.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        const batches = localLines(context).some((line) => (segments(line) ?? []).some(({ words, piped }) => piped && nameOf(words[0] ?? '') === 'xargs'))
        if (batches) context.report({ node: document })
      },
    }
  },
})

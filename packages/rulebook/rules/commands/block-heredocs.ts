import { defineRule } from '@cmodjs/rulebook'
import { localLines } from '../../lib/shell.ts'

const HEREDOC = /<<-?\s*(['"]?)(\w+)\1[^\n]*\n(?:.*\n)?\s*\2\s*$/ms

export default defineRule({
  description: "Don't carry a file in a heredoc.",
  message: 'Write the file with the Write tool, and pass its path.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (localLines(context).some((line) => HEREDOC.test(line))) context.report({ node: document })
      },
    }
  },
})

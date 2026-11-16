import { defineRule } from '@cmodjs/rulebook'
import { localLines, tokens, unreadable } from '../../lib/shell.ts'

export default defineRule({
  description: "Don't hide what a command runs.",
  message: 'Inline code, a script that is not our own tooling, and an unbalanced quote each leave the real command unknowable. Run the operation as a plain command.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (unreadable(context) !== '' || localLines(context).some((line) => tokens(line) === undefined)) context.report({ node: document })
      },
    }
  },
})

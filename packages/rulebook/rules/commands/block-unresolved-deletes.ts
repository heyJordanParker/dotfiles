import { defineRule } from '@cmodjs/rulebook'
import { deleteRefusal } from '../../lib/delete.ts'

export default defineRule({
  description: "Don't delete through a pipe, a substitution, or a variable.",
  message: 'Run rm against a literal path inside an allowed folder, or delete the file by hand.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (deleteRefusal(context.text, context.call?.cwd ?? '') === 'unresolved') context.report({ node: document })
      },
    }
  },
})

import { defineRule } from '@cmodjs/rulebook'
import { deleteRefusal } from '../../lib/delete.ts'

export default defineRule({
  description: 'Delete only inside the dotfiles, ~/Developer, ~/Downloads, ~/Desktop, ~/conductor, ~/.claude, or /tmp.',
  message: 'Delete anything else by hand.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (deleteRefusal(context.text, context.call?.cwd ?? '') === 'outside') context.report({ node: document })
      },
    }
  },
})

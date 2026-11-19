import { defineRule } from '@cmodjs/rulebook'
import { deletesOutsideAllowedFolders, skipsTheTrash } from '../../lib/delete.ts'

export default defineRule({
  description: 'Delete only with an rm your shell runs, inside the dotfiles, ~/Developer, ~/Downloads, ~/Desktop, ~/conductor, ~/.claude, or /tmp.',
  message: 'Run rm yourself on paths you name, or delete it by hand.',
  files: ['@command'],
  create(context) {
    if (skipsTheTrash(context) || deletesOutsideAllowedFolders(context)) context.report({ node: context.sourceCode.ast })
    return {}
  },
})

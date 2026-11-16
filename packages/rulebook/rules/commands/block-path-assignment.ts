import { defineRule } from '@cmodjs/rulebook'
import { headAndArgs, segments } from '../../lib/shell.ts'

const TIED = /^(path|cdpath|fpath|manpath)=/

export default defineRule({
  description: "Don't assign to path, cdpath, fpath, or manpath.",
  message: 'In zsh each one is tied to its uppercase array, so a plain string replaces the array and breaks command lookup for the rest of the shell. Rename the variable, such as target_path.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        const clobbers = (segments(context.text) ?? []).some(({ words }) => headAndArgs(words)[0] === '' && words.some((word) => TIED.test(word)))
        if (clobbers) context.report({ node: document })
      },
    }
  },
})

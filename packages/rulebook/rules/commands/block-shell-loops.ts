import { defineRule } from '@cmodjs/rulebook'
import { localLines, nameOf, segments } from '../../lib/shell.ts'

const LOOP_WORDS: ReadonlySet<string> = new Set(['for', 'while', 'until', 'select', 'do', 'done'])

export default defineRule({
  description: "Don't loop inside a shell call.",
  message: 'Run the command once per item, one call each.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        const loops = localLines(context).some((line) => (segments(line) ?? []).some(({ words }) => LOOP_WORDS.has(nameOf(words[0] ?? ''))))
        if (loops) context.report({ node: document })
      },
    }
  },
})

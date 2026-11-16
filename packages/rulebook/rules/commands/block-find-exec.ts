import { defineRule } from '@cmodjs/rulebook'
import { headAndArgs, localLines, segments } from '../../lib/shell.ts'

const ACTIONS: ReadonlySet<string> = new Set(['-exec', '-execdir', '-ok', '-okdir'])

const ACTION_ENDS: ReadonlySet<string> = new Set(['{}', ';', '\\;', '+'])

export default defineRule({
  description: "Don't run a command per match with find -exec.",
  message: 'Find the matches, then act on them one at a time.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        const runsPerMatch = localLines(context).some((line) => (segments(line) ?? []).some(({ words }) => {
          if (!['find', 'fd'].includes(headAndArgs(words)[0])) return false
          const action = words.findIndex((word) => ACTIONS.has(word))
          return action >= 0 && words.slice(action + 1).some((word) => !ACTION_ENDS.has(word))
        }))
        if (runsPerMatch) context.report({ node: document })
      },
    }
  },
})

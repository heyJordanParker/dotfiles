import { defineRule } from '@cmodjs/rulebook'
import { headAndArgs } from '../../lib/shell.ts'

const SINKS: ReadonlySet<string> = new Set(['sort', 'uniq'])

const SEARCHERS: ReadonlySet<string> = new Set(['rg', 'grep', 'egrep', 'fgrep', 'ag', 'find', 'fd'])

const BOUNDS: readonly string[] = ['-c', '--count', '--count-matches', '-m', '--max-count', '-l', '--files-with-matches']

export default defineRule({
  description: "Don't pipe an unbounded search into sort or uniq.",
  message: 'sort holds its whole input in memory, about 4.5 times its size. Bound the search with rg -c, rg --max-count <n>, or head before the sort.',
  files: ['@command'],
  create(context) {
    const { commands } = context
    return {
      Command(command) {
        if (command.input === undefined) return
        const isPrefix = headAndArgs([command.program])[0] === ''
        const reader = isPrefix ? commands[commands.indexOf(command) + 1] : command
        const source = commands[command.input]
        if (reader === undefined || source === undefined || !SINKS.has(reader.program) || !SEARCHERS.has(source.program)) return
        if (!source.args.some((arg) => BOUNDS.some((bound) => arg === bound || arg.startsWith(`${bound}=`)))) context.report({ node: command })
      },
    }
  },
})

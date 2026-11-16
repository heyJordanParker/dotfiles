import { existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { defineRule } from '@cmodjs/rulebook'

const FORCE_FLAGS: readonly string[] = ['-f', '--force', '--discard-changes']

const BRANCH_FLAGS: readonly string[] = ['-b', '-B', '--orphan']

export default defineRule({
  description: "Don't check out files from git.",
  message: 'Undo specific lines with the Edit tool. To prove a failure is pre-existing, report the command and its red output instead.',
  files: ['@command'],
  create(context) {
    return {
      Document(document) {
        if (context.commands.length === 0 && /git\s+checkout/.test(context.text)) context.report({ node: document })
      },
      Command(command) {
        if (command.program !== 'git' || command.args[0] !== 'checkout') return
        const args = command.args.slice(1)
        if (args.includes('--ours') || args.includes('--theirs') || args.some((arg) => FORCE_FLAGS.includes(arg))) return
        const folder = resolve(context.call?.cwd ?? context.root, command.folder)
        if (overwritesFiles(args, folder)) context.report({ node: command })
      },
    }
  },
})

function overwritesFiles(args: readonly string[], folder: string): boolean {
  if (args.includes('--') || args.some((arg) => arg.startsWith('--pathspec-from-file'))) return true
  if (args.some((arg) => BRANCH_FLAGS.includes(arg))) return false
  const positional = args.filter((arg) => !arg.startsWith('-'))
  const [first] = positional
  return positional.length > 1 || (first !== undefined && (/[./*?[]/.test(first) || existsSync(resolve(folder, first))))
}

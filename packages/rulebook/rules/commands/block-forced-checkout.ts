import { defineRule } from '@cmodjs/rulebook'

const FORCE_FLAGS: readonly string[] = ['-f', '--force', '--discard-changes']

export default defineRule({
  description: "Don't force a checkout or a switch.",
  message: "A forced checkout throws away every uncommitted edit in the tree, other agents' work included. Run it without the force flag, and git refuses it when it would overwrite an edit.",
  files: ['@command'],
  create(context) {
    return {
      Command(command) {
        if (command.program !== 'git' || !['checkout', 'switch'].includes(command.args[0] ?? '')) return
        const args = command.args.slice(1)
        const resolvesConflict = command.args[0] === 'checkout' && (args.includes('--ours') || args.includes('--theirs'))
        if (!resolvesConflict && args.some((arg) => FORCE_FLAGS.includes(arg))) context.report({ node: command })
      },
    }
  },
})

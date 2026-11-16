import { defineRule } from '@cmodjs/rulebook'
import { isDispatched } from '../../lib/session.ts'

const NAME = /^[A-Za-z0-9_@][A-Za-z0-9_@-]*$/

export default defineRule({
  description: "Don't change branches in a Subagent.",
  message: 'The worktree is shared, so a branch change moves HEAD under the main session and every sibling. Say in your report that the work needs a branch change.',
  files: ['@command'],
  create(context) {
    const call = context.call
    if (call === undefined || !isDispatched(call) || context.commands.some(({ args }) => args.includes('--'))) return {}
    return {
      Command(command) {
        if (command.program !== 'git') return
        const [subcommand, first = ''] = command.args
        const changes = subcommand === 'switch' || (subcommand === 'branch' && /^-[mMdD]/.test(first)) || (subcommand === 'checkout' && (/^-[bB]/.test(first) || NAME.test(first)))
        if (changes) context.report({ node: command })
      },
    }
  },
})

import { defineRule } from '@cmodjs/rulebook'
import { isWritableWhileProposing, WRITE_TOOLS, writtenFile } from '../../lib/proposal.ts'
import { ownerSession, stage } from '../../lib/session.ts'
import { unreadable } from '../../lib/shell.ts'

const MUTATES_EVERY_ARGUMENT: ReadonlySet<string> = new Set(['mkdir', 'rmdir', 'unlink', 'shred', 'mkfifo', 'mknod'])

const MUTATES_ITS_TARGET: ReadonlySet<string> = new Set(['chmod', 'chown', 'chgrp'])

export default defineRule({
  description: "Don't change code while the session is proposing.",
  message: 'The Architect moves the session to executing by typing /execute, /orchestrate, or /build. Plans, shaping docs, and Evidence stay writable in docs/plans/, docs/shaping/, docs/agents/, and /tmp/.',
  files: ['**', '@command'],
  create(context) {
    const call = context.call
    if (call === undefined || ownerSession(call) === '' || stage(call) !== 'propose') return {}
    if (WRITE_TOOLS.has(call.tool)) {
      return {
        Document(document) {
          if (!isWritableWhileProposing(writtenFile(context), call.cwd)) context.report({ node: document })
        },
      }
    }
    if (context.path !== '@command') return {}
    return {
      Document(document) {
        const cannotName = unreadable(context) !== ''
        const writes = (context.shell?.writes ?? []).some((target) => !isWritableWhileProposing(target, call.cwd))
        if (cannotName || writes) context.report({ node: document })
      },
      Command(command) {
        const positional = command.args.filter((arg) => !arg.startsWith('-'))
        const targets = MUTATES_EVERY_ARGUMENT.has(command.program) ? positional : MUTATES_ITS_TARGET.has(command.program) ? positional.slice(1) : command.program === 'install' ? positional.slice(-1) : []
        const changesTree = command.program === 'git' && ['rm', 'mv'].includes(command.args[0] ?? '') && !command.args.includes('--cached')
        if (changesTree || targets.some((target) => !isWritableWhileProposing(target, `${call.cwd}/${command.folder}`))) context.report({ node: command })
      },
    }
  },
})

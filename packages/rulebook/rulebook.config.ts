import { defineConfig, recommended } from '@cmodjs/rulebook'

export default defineConfig({
  extends: [recommended],
  plugins: { global: './rules' },
  files: [
    '**/CLAUDE.md',
    '**/Claude.md',
    '**/AGENTS.md',
    '**/Agents.md',
    '**/Domain.md',
    '**/rules/**/*.md',
    '**/agents/*.md',
    '**/commands/**/*.md',
    '**/skills/**/*.md',
  ],
  probes: ['probes/*.md'],
  timeout: 60_000,
  messages: { spawn: { tools: ['Agent', 'Workflow', 'RemoteTrigger', '*spawn_agent'] } },
})

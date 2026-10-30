import { defineConfig } from '@cmodjs/plint'

export default defineConfig({
  plugins: { global: './rules' },
  paths: [
    '**/CLAUDE.md',
    '**/Claude.md',
    '**/AGENTS.md',
    '**/Agents.md',
    '**/Domain.md',
    '**/rules/**/*.md',
    '**/agents/**/*.md',
    '**/commands/**/*.md',
    '**/skills/**/*.md',
  ],
  probes: ['probes/*.md'],
  editTimeout: 60_000,
})

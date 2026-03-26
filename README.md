# Talent Tree

Overpowered Claude Code talents to power level your software architecture.

## Install Talents

Requires [Claude Code](https://docs.anthropic.com/en/docs/claude-code).

```bash
/plugin marketplace add heyJordanParker/dotfiles
/plugin install talents@talent-tree
```

### Talents

Invoked as `/talents:<name>`:

- **agent-browser** — Automate browser interactions, web testing, screenshots, data extraction (requires [agent-browser](https://www.npmjs.com/package/agent-browser))
- **architecture** — Present architectural options with tradeoffs
- **breadboarding** — Map workflows into affordance tables
- **breadboard-review** — Find design smells in breadboards
- **cc** — Work with Claude Code skills, hooks, settings
- **codebase-exploration** — Explore codebase structure and dependencies
- **commit-message** — Structured commits with type prefix and file tree
- **debug** — Systematic debugging with root cause tracing
- **design** — UI components, styling, and interaction decisions
- **diagram** — Generate Excalidraw diagrams on a live canvas (requires [drawbridge](https://github.com/alexknowshtml/drawbridge))
- **naming** — Consistent naming for variables, files, classes, everything
- **debate** — N independent architects debate architectural options through structured rounds with cross-pollination
- **execute-plan** — Orchestrate implementation plans with persistent team, validation gates per slice
- **independent-review** — N identical parallel agents for consensus through redundancy
- **ledger** — Review and update Claude.md files for template compliance
- **modeling** — Transform shaped parts into concrete models (DB schema, UX flows, architecture)
- **pcc** — Add pros/cons/confidence to any prompt
- **personas** — 5 parallel persona agents for diverse perspectives
- **plan** — Plan features using structured format
- **pragmatic-engineering** — KISS-driven planning and review
- **review-plan** — Review planning artifacts with 5 parallel specialized agents
- **review** — Parallel code reviewers on uncommitted changes
- **shaping** — Collaborate on problem definition and solution options
- **show-architecture** — Annotated file trees inline
- **slicing** — Break modeled features into vertical implementation slices with acceptance criteria
- **subagents** — Framework for dispatching and managing subagents
- **user-testing** — Trace real user flows through code changes, find gaps
- **using-git-worktrees** — Isolated git worktrees for feature work
- **verification-before-completion** — Evidence before assertions
- **working-with-markdown-files** — Rules for markdown editing
- **writing-tests** — Behavior-driven tests, not implementation tests

### Commands

Also invoked as `/talents:<name>`:

- **ask** — Break complex scenarios into decision questions with 4+ options
- **brainstorm** — Interactive design refinement using Socratic method
- **bug-hunt** — Review code for bugs, logic errors, security vulnerabilities
- **commit** — Validated commit with tests and comprehensive review
- **docs-review** — Review Claude documentation with parallel subagents
- **pcc** — Add pros/cons/confidence to any prompt
- **plan** — Plan a feature using structured format
- **q** — Answer questions only, no actions
- **retro** — Analyze conversation history for patterns and improvements
- **wtf** — Hard reset, redo the last task correctly

### Hooks

The plugin includes hooks that run automatically to keep Claude disciplined:

- **session-start** — Injects ground rules (no sycophancy, literal execution, read before acting)
- **retro-reminder** — Nudges to run `/retro` if 3+ days since last one
- **block-git-revert** — Blocks `git reset`, `git restore`, `git checkout -- <file>`. Forces manual execution.
- **block-unsafe-delete** — Whitelists `rm` to specific directories only. Everything else blocked.
- **pre-edit** — "Did you read this file?" checklist before any edit
- **ask-user-question** — Enforces research-first, 4+ options, self-contained questions
- **sync-shaping** — Ripple-check reminders when editing shaping documents
- **user-prompt-submit** — Pre-response checklist (read the code, question vs instruction, test your changes)

### Safe Delete

Claude loves running `rm -rf`. Protect yourself by replacing `rm` with [trash](https://github.com/ali-rantakari/trash) so deleted files go to macOS Trash instead of being permanently destroyed:

```bash
brew install trash
```

Then alias `rm` to `trash` in your shell profile:

```bash
cat >> ~/.zshenv << 'EOF'
# Safe delete - moves to Trash instead of permanent deletion
rm() {
  local args=()
  for arg in "$@"; do
    [[ "$arg" =~ ^-[rRfidv]+$ ]] && continue
    args+=("$arg")
  done
  trash "${args[@]}"
}
EOF
```

This silently strips `rm` flags (`-rf`, `-i`, etc.) and sends files to Trash. Claude thinks it's deleting; you can undo from Trash.

---

## Dotfiles

macOS dotfiles managed with [GNU Stow](https://www.gnu.org/software/stow/).

### New Machine Setup

```bash
curl -fsSL https://raw.githubusercontent.com/heyJordanParker/dotfiles/master/setup.sh | bash
```

This installs Xcode CLI tools, Homebrew, all packages from `Brewfile`, bun, and symlinks configs.

### Manual Usage

```bash
# Add a new config
cd ~/dotfiles
mkdir -p newapp/.config/newapp
mv ~/.config/newapp/config newapp/.config/newapp/
stow newapp

# Remove symlinks
stow -D newapp

# Re-link after changes
stow -R newapp
```

### Structure

Each top-level directory is a stow package. Files inside mirror their target location relative to `$HOME`.

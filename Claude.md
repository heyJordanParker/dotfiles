# Dotfiles

## Why

Reproducible macOS environment setup and Claude Code plugin distribution from a single repository.

## What

GNU Stow-managed dotfiles, organized under `packages/`. Each subdirectory of `packages/` is a stow package whose contents are exactly what should land at the package's stow target. The `claude` package doubles as the source for the Claude Code plugin marketplace.

### Requirements

- Every package contains exactly the files that should land in its target — no inner mirror wrappers (`.config/<tool>/`, `.<tool>/`)
- Packages & tools should be self-contained. Each package owns its own files, configs, ignores — prefer a nested `.gitignore` inside the package over editing the root `.gitignore` to make sure that cleaning up a package is as simple as deleting it.
- Plugin marketplace must distribute skills, hooks, and commands without leaking local config
- Must restow with the correct `-t` target after editing — `stow -t <target> <pkg>`

### Boundaries

- Never commit `settings.json`, `Claude.md`, or other local config to plugin distribution — these are local only
- Never put rules in plugin manifest — not supported by plugin schema
- Never break the per-package target mapping in `scripts/stow.py` — each package needs the correct target so contents land where they belong
- Never bind custom keybindings to the macOS terminal defaults listed in the Reference section — they're system-wide and overriding them breaks expected shell behavior

## Architecture

```
dotfiles/
├── .claude/                          # gitignored — Claude Code's local working dir for this repo
├── .claude-plugin/
│   └── marketplace.json              # source: "./packages/claude"
├── packages/                         # stow source dir; each child is a stow package
│   ├── agents/                       # → ~/.agents/      (source of truth: skills/, agents/*.md, commands/*.md, hooks/*.py, tooling/<name>/)
│   ├── claude/                       # → ~/.claude/      (settings, rules, Claude.md; skills/ agents/ commands/ → ../agents/*)
│   ├── codex/                        # → ~/.codex/       (config, rules, Agents.md; agents/ → ../agents/agents, prompts/ → ../agents/commands)
│   ├── ssh/                          # → ~/.ssh/         (config)
│   ├── bin/                          # → ~/.local/bin/   (custom shell scripts)
│   ├── starship/                     # → ~/.config/      (starship.toml)
│   ├── git/                          # → ~/              (.gitconfig)
│   ├── hyprspace/                    # → ~/              (.hyprspace.toml)
│   ├── npm/                          # → ~/              (.npmrc)
│   ├── tmux/                         # → ~/              (.tmux.conf, .tmux/)
│   ├── zsh/                          # → ~/              (.zshrc, .zprofile, .zshenv, .zsh_completions.zsh)
│   ├── atuin/, bat/, borders/, btop/, bun/, delta/, ghostty/, hunk/,
│   ├── karabiner/, lazygit/, nvim/, opencode/, superfile/, zed/, zellij/   # → ~/.config/<pkg>/
├── scripts/                          # repo automation (python, stdlib at runtime): sync.py = restow + generate codex agents + generate hook wiring
├── tools/                            # native CLIs built from source, not stowed: tracer (the `trace` binary), prompt-reviewer
├── tests/                            # pytest suite for the Python hooks + build scripts (tests/hooks/); run via uv
├── Brewfile
├── Claude.md                         # this file — repo project docs
├── README.md
├── pyproject.toml                    # uv-managed dev env (pytest, ruff) for the Python automation — dev-only, gitignored .venv
├── uv.lock
├── setup-secrets.sh
├── setup.sh
├── .gitignore
└── .stow-local-ignore
```

### Cross-tool sharing (agents = source of truth)

- **Skills source of truth: `packages/agents/skills/<name>/SKILL.md`** — real files, open Agent Skills format. One set, read by every tool. Stow lays down the homedir links and a single in-repo symlink bridges Claude — no sync scripts.
  - `packages/agents/` → stow → `~/.agents/skills/<name>`. `setup.sh` pre-creates `~/.agents/skills` as a real dir so stow links its children per-skill rather than folding the whole dir — that lets foreign tools' skills coexist there. Codex and every Agent-Skills-standard adopter read `~/.agents/skills` natively.
  - `packages/claude/skills → ../agents/skills` — a committed relative symlink. `stow -t ~/.claude claude` makes `~/.claude/skills → packages/claude/skills → ../agents/skills`, so Claude Code resolves the same source. Claude scans only `~/.claude/skills`; it does NOT read `~/.agents/skills` (confirmed empirically).
  - Codex needs no skills mirror — it reads `~/.agents/skills` directly, so there is no `packages/codex/skills` and no sync script.
- **Agents & commands source of truth: `packages/agents/agents/<name>.md`, `packages/agents/commands/<name>.md`** — same model as skills. `packages/claude/agents → ../agents/agents` and `packages/claude/commands → ../agents/commands` bridge Claude; `packages/codex/prompts → ../agents/commands` bridges Codex (it reads command `.md` verbatim — frontmatter is tolerated). Codex subagents need a format transform, not a symlink: `scripts/sync.py` generates two siblings — `<name>.toml` (codex subagent shape — `name`, `description`, body → `developer_instructions`; our `model`/`tools`/`skills`/`color`/`memory` dropped) and `<name>.prompt.md` (the frontmatter-stripped body, usable as base instructions via codex's `model_instructions_file`) — and `packages/codex/agents → ../agents/agents` exposes both. Codex boots every session as the CTO: `packages/codex/config.toml` sets `model_instructions_file = ~/.agents/agents/cto.prompt.md`, which replaces codex's built-in base prompt (a separate request field from the global Claude.md, so both load together — verified against codex 0.137 source). Both generated artifacts are gitignored (`packages/agents/.gitignore`) and rebuilt by `sync.py`, so the `.md` stays the only tracked source. Codex custom prompts are deprecated in favor of skills (already shared), so the command bridge is a thin symlink, not a generator.
- **Agent tooling: `packages/agents/tooling/<name>/`** — a buildable kit an agent invokes (e.g. the `plan-visualizer` agent's React/Vite review-renderer). Stows whole-dir to `~/.agents/tooling/<name>/`, and the agent references it by that stable absolute path — agents do NOT receive the per-skill "Base directory" line a skill gets, so a relative path would resolve against the user's project, not the kit. This is the same absolute-path-into-stow model as `~/.agents/hooks/<module>.py` and `~/.agents/agents/<name>.prompt.md`. First-use dependency setup is a guarded `[ -d node_modules ] || npm ci` from the committed `package-lock.json`; the kit's own `.gitignore` ignores `node_modules`, build artifacts, and per-run authored inputs. Local-only — agents are not plugin-distributed (`marketplace.json` `"agents": []`), so neither is their tooling.
- `packages/codex/Agents.md → ../../../.claude/Claude.md` — Codex reads the same global Claude.md as Claude does (post-stow user-global)
- Codex's bundled `.system/` skills (`imagegen`, `openai-docs`, `plugin-creator`, `skill-creator`, `skill-installer`) are not tracked in dotfiles. Codex writes them to its own `~/.codex/skills/.system/` on each restart — separate from the shared `~/.agents/skills`.
- Skill manifest casing: every manifest is named `SKILL.md` (uppercase) — the single source of truth. Codex's loader byte-matches the literal `SKILL.md` and only auto-discovers under that exact name (confirmed empirically); Claude Code finds the manifest either casing on the case-insensitive filesystem. The uppercase name is what makes the shared set surface as real skills in both tools, not just Claude.
- **Hooks: `packages/agents/hooks/<module>.py`** — shared Python hooks drawn from by both Claude and Codex (same source-of-truth model as skills), wired into both harnesses by absolute `~/.agents/hooks/<module>.py` path. The wiring is **generated, not hand-written**: each hook declares a module-level `BINDING = {"events": {...}, "harness": "all" | "claude" | "codex"}`, and `scripts/hooks.py` reads every `BINDING` statically (never imports the hook) and rewrites the managed hook entries of `packages/claude/settings.json` and the `[[hooks.*]]` + `[hooks.state]` region of `packages/codex/config.toml`. A managed entry is a `type: command` hook invoking `~/.agents/hooks/<module>.py`; every other entry (inline `type: prompt` gates, third-party shell glue) and every non-hook section is preserved untouched. To change where a hook fires, edit its `BINDING` and run `sync.py` — never hand-edit the generated regions. The `harness` field decides which config a hook lands in: `all` → both, `claude` → settings.json only, `codex` → config.toml only. Codex runs nearly the whole set — the recording/classification spine, the command and session-state guards, the doc/context injectors, the validators, `sync_shaping`, and `auto_approve_permissions`. `enforce_background_codex_run` forces `codex-run` Bash calls into the background, governing only that wrapper, never raw `codex exec`. Two single-purpose doc injectors split the work: `inject_docs.py` (both harnesses, PreToolUse `Bash`) ensures a path-taking `trace <subcmd> <path>` command has its target's project docs in context, blocking the command if `trace docs` fails. `inject_rules.py` (Codex-only, since Claude loads Claude.md itself) injects the nearest Claude.md — repo-root rules on SessionStart with a `trace docs reset` on `clear`/`compact`, and the touched file's rules on PreToolUse `Read`/`Write`/`Edit`/`apply_patch`. Both emit a `hookSpecificOutput.additionalContext` envelope (Codex 0.137+ rejects a raw `trace docs` JSON object on SessionStart with "invalid session start JSON output" — it must be wrapped). Excluded from Codex (their `BINDING` carries `harness: "claude"`): the guards for Claude-only tools and events (Agent / EnterWorktree / TeamDelete / ExitPlanMode) — `block_builtin_subagents`, `block_worktree_isolation`, `block_enter_worktree`, `block_team_deletion`, `enforce_solo_mode`, `transition_state_after_plan`, `validate_plan_quality`, and `archive_subagent_log` — which Codex has no event to fire on. Codex resolves the session from env, so the hooks export the payload's `session_id`/`agent_id` as `AGENT_SESSION_ID`/`TRACER_AGENT_ID` — `AGENT_SESSION_ID` is the harness-neutral session carrier `trace` and our own tooling resolve first (before `CODEX_THREAD_ID` and `CLAUDE_CODE_SESSION_ID`). Hook trust is per-command-string in `config.toml`'s `[hooks.state]` (also generated by `hooks.py`, hashed over each emitted command), so editing a hook's Python needs no re-approval — only changing the wired command does. Local-only; never plugin-distributed. The only shell hooks left in `packages/claude/hooks/` are the five the plugin marketplace distributes to external installs that have no Python layer (plus one third-party vendor script) — see `packages/claude/hooks/Claude.md`.

## Workflow

### When You Don't Need to Restow

Editing **content** of an already-stowed file works through the symlink — the link at `$HOME` points at the source, so edits land in the source and stay live without any stow command. Day-to-day edits (settings.json tweak, hook script change, new skill text) need nothing further.

### When You Do Need to Restow

Restow on filesystem-shape changes inside a package — these aren't picked up by existing symlinks:

- **Adding** a file/dir to a package
- **Removing** a file/dir from a package (the dangling symlink at the target stays until stow cleans it)
- **Renaming or restructuring** anything inside a package
- **First-time stow** of a brand-new package

Per-package restow (`<repo>` is wherever you cloned the dotfiles):

```bash
cd <repo>/packages
stow -R -t <target> <pkg>     # e.g., stow -R -t ~/.claude claude
```

The target for each `<pkg>` is the one defined in `setup.sh` — see the per-package mapping in the "Stow Targets" section under How.

Whole-repo restow (after multi-package changes): run `python3 scripts/sync.py` from the clone (or re-run `setup.sh`, which calls it). `sync.py` restows every package, regenerates the codex agent artifacts, and regenerates the hook wiring in `settings.json` + `config.toml`. The package→target mapping lives in `scripts/stow.py` — the single source.

### Adding a New Package

1. Create the package directory inside `packages/`
2. Add the actual config files at the package root (no inner wrappers — the package contents are exactly what should land at the target)
3. Add the package → target entry to `scripts/stow.py` (`TARGETS`, or the `CONFIG` list for a `~/.config/<pkg>/` tool)
4. Run `python3 scripts/sync.py` — it creates the target dir and stows the package

### Adding/Renaming Files Inside an Existing Package

After the change, restow that one package: `stow -R -t <target> <pkg>`. No edit to `setup.sh` is needed — the package is already in its stow block.

### Removing a Package

1. Unstow it: `stow -D -t <target> <pkg>`
2. Delete the package directory under `packages/`
3. Remove its line from `setup.sh`

### Installing CLI Tools

1. Add to `Brewfile` (appropriate section)
2. If config needed: create a package dir under `packages/`, add config files at package root
3. If wrapper needed (secrets, env vars): add a script to `packages/bin/` and restow `bin`
4. Stow new/modified packages with the correct `-t`

### Python Tools (pipx)

```bash
pipx install <package>
```

### Editing the Stow Mapping

`scripts/stow.py` holds the mapping — `TARGETS` (package → target dir) plus the `CONFIG` list (each lands in `~/.config/<pkg>/`). When the mapping changes, edit `stow.py`; it's the single source. `setup.sh` and the pre-commit hook both restow through `scripts/sync.py`, which calls it.

## How

### Repo automation (`scripts/`)

The automation runtime is Python, stdlib-only — it runs under the system `python3` by absolute path, ships nothing to plugin consumers, and matches the hook convention. `pyproject.toml` + `uv.lock` declare a separate uv-managed dev environment (pytest, ruff) for testing and linting that automation; it lives in the gitignored `.venv/` and is never imported at runtime. `setup.sh` is the only bash file and just bootstraps (Homebrew, builds), then guards that `python3` exists and hands off.

- `sync.py` — the one maintenance entry point: restow every package, regenerate the codex agent artifacts (`agents.generate`), and regenerate the hook wiring (`hooks.generate`). Idempotent. Called by `setup.sh` and the pre-commit hook so there's one implementation, not two.
- `stow.py` — the package→target mapping and restow (`stow -R` per package).
- `agents.py` — transforms `packages/agents/agents/*.md` into the two codex siblings (`<name>.toml`, `<name>.prompt.md`) beside each.
- `hooks.py` — reads each hook's `BINDING` declaration and rewrites the managed hook entries of `packages/claude/settings.json` and the `[[hooks.*]]` + `[hooks.state]` region of `packages/codex/config.toml`. Everything else in those files is preserved untouched. See the Hooks bullet under Cross-tool sharing for the model.
- `frontmatter.py` — shared `---` frontmatter + body parser.
- `git-hooks/pre-commit` — runs `sync.py` on commit; never blocks. Installed by `setup.sh` via `git config core.hooksPath scripts/git-hooks`, so it is version-controlled and survives a clone.

**Commit-time gotcha:** because the pre-commit hook runs `sync.py`, every commit rewrites `settings.json` and `config.toml` to the generator's canonical form. If they weren't already canonical when you committed, the rewrite leaves them dirty in the working tree *after* the commit lands. Before a multi-commit session, run `python3 scripts/sync.py` once up front so the generated files settle — then each subsequent commit is a no-op for the generator and leaves nothing dirty.

### Stow Targets — the explicit per-package mapping

`scripts/stow.py` restows once per package, into these targets:

- Home root (`~/`): `git`, `hyprspace`, `npm`, `tmux`, `zsh`
- Single-segment dirs: `agents` → `~/.agents/`, `claude` → `~/.claude/`, `codex` → `~/.codex/`, `ssh` → `~/.ssh/`
- Special targets: `bin` → `~/.local/bin/`, `starship` → `~/.config/`
- `~/.config/<tool>/` group (loop): `atuin`, `bat`, `borders`, `btop`, `bun`, `delta`, `ghostty`, `hunk`, `karabiner`, `lazygit`, `nvim`, `opencode`, `superfile`, `zed`, `zellij`

### Plugin Marketplace

Users install with:
```
/plugin marketplace add heyJordanParker/dotfiles
/plugin install talents@talent-tree
```

**Distributed:** Skills (physically `packages/agents/skills/`, reached through the committed `packages/claude/skills → ../agents/skills` symlink that the marketplace entry's `"skills": ["./skills/"]` resolves), Commands (physically `packages/agents/commands/`, reached through the committed `packages/claude/commands → ../agents/commands` symlink — same dereference-on-copy as skills), Hooks (`packages/claude/hooks/hooks.json` with `${CLAUDE_PLUGIN_ROOT}` paths). Plugin packaging **dereferences** the skills symlink when copying the source into the plugin cache, so consumers receive every skill as real `SKILL.md` files — verified against a clean install (Claude Code 2.1.162): the installed plugin's `skills/` is a real directory containing all 34 skills, not a dangling link.

**Local-only:** Rules, settings, agents, `settings.json`, `Claude.md`, tmux hooks. Codex's `.system/` defaults are not in dotfiles at all — Codex creates them at `~/.codex/skills/.system/` on each restart, outside the plugin source tree.

### Dual Hooks Setup

The hooks Claude (and Codex) run are Python, shared from `packages/agents/hooks/<module>.py`. The wiring in both `settings.json` (Claude) and `config.toml` (Codex) is generated from each hook's `BINDING` by `scripts/hooks.py` — not hand-edited. Codex gets nearly all of them (all but the Claude-only-tool guards, whose `BINDING` carries `harness: "claude"`). See the cross-tool sharing section above and `packages/claude/hooks/Claude.md` for the full wiring model.

The plugin marketplace can't run Python hooks, so it ships shell copies. `packages/claude/hooks/hooks.json` is the plugin wiring (`${CLAUDE_PLUGIN_ROOT}` paths); it references exactly five shell hooks in `packages/claude/hooks/`: `block-git-revert.sh`, `block-unsafe-delete.sh`, `validate-planning-docs.sh`, `validate-plan-quality.sh`, `sync-shaping.sh`. Tracer hooks, the intent classifier and its dependent state guards, subagent-only hooks, and Claude-only-tool guards are local-only and never plugin-distributed — plugin users get the tracer binary as a command, not its hooks.

When changing a plugin-distributed hook, update both the Python source and its shell copy here, and keep `hooks.json` in sync. When changing a local-only hook, only the Python source and the `settings.json` / `config.toml` wiring matter.

### Expanding the Marketplace

- **Add skill:** Create `packages/agents/skills/<name>/SKILL.md` (the source of truth) — auto-discovered by all tools via the stow + `claude/skills → ../agents/skills` links
- **Add command:** Create `packages/agents/commands/<name>.md` (the source of truth) — auto-discovered by Claude via the `claude/commands → ../agents/commands` link and by Codex via `codex/prompts → ../agents/commands`
- **Add hook:** Add the Python module to `packages/agents/hooks/` with a `BINDING` declaring its events and `harness` (`all`, or `claude` if it guards a Claude-only tool/event Codex can't fire on), then run `python3 scripts/sync.py` to generate the wiring into `settings.json` and `config.toml` — don't hand-edit those. To make it a plugin hook too, add a shell copy under `packages/claude/hooks/` and reference it in `hooks/hooks.json`
- **Bump version:** Update `version` in `marketplace.json` plugin entry — required for users to get updates
- **Validate:** `claude plugin validate .` from repo root
- **Test locally:** `/plugin marketplace add ./` then `/plugin install talents@talent-tree`

### Plugin Distribution Limitations

- `strict: false` — marketplace entry defines all components, no `plugin.json` needed
- Entire `./packages/claude` directory gets copied to plugin cache (extra files are inert); the copy dereferences the `skills → ../agents/skills` symlink, so the out-of-tree skill set lands in the cache as real files rather than a broken link
- Plugin consumers don't get rules or settings — those go in their own Claude.md/settings

## Reference

### macOS Terminal Keybindings (system defaults — don't override)

#### Ctrl (readline/shell)
- ^a — beginning of line
- ^e — end of line
- ^b — back one char
- ^f — forward one char
- ^d — delete forward
- ^h — delete backward
- ^k — kill to end of line
- ^u — kill whole line
- ^w — kill word backward
- ^y — yank (paste killed text)
- ^t — transpose chars
- ^p — previous history
- ^n — next history
- ^r — reverse search history
- ^l — clear screen
- ^c — interrupt
- ^z — suspend
- ^i — tab (same keycode)
- ^j — newline (same keycode)
- ^m — return (same keycode)

#### Alt/Option (word movement)
- ~b — back one word
- ~f — forward one word
- ~d — delete word forward
- ~Delete — delete word backward
- ~Enter — insert newline
- ~Tab — insert tab
- ~Esc — complete

#### Cmd — typically handled by the terminal app, not the shell
- Cmd+c — copy
- Cmd+v — paste
- Cmd+a — select all
- Cmd+. — cancel

### ~/Developer Directory

Two subdirectories with distinct purposes:

- **`references/`** — Temporary repos cloned for reading/studying code. Not synced or automated. Clone what you need, delete when done.
- **`services/`** — Repos we clone and run. Setup automated in `setup.sh` so any machine can reproduce.

#### Current Services

- **drawbridge** — Real-time diagram server for AI agents. Pushes simplified elements via HTTP → live Excalidraw canvas in browser.
  - Repo: `heyJordanParker/drawbridge`
  - Setup: `npm install && npm run build && npx playwright install chromium`
  - Run: `npm start` (API + WebSocket + static frontend on :3062)
  - Open: `http://localhost:3062/#session-name`
  - Skill: `/diagram` (installed at `~/.claude/skills/diagram/SKILL.md`)

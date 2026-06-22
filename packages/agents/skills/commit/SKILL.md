---
name: commit
description: |
  Mandatory contract for every commit. Loads automatically when the classifier authorizes a commit — it injects "Skills to execute: /commit" whenever the user asks to commit ("/commit", "commit this", "create a commit"). Holds the whole commit job: stage, write the message, commit, verify, then suggest which session notes should become permanent — plus the commit-message format (type prefix, what+why body, file tree). TRIGGER on every commit-authorized turn, or when the user asks to write or revise a commit message. DO NOT TRIGGER when the user has not asked to commit — applying changes, deploying, shipping, or replacing files are not commit requests.
---

# Commit

You are committing work the architect approved. The cto prompt governs reading before claiming, proving it ran, and holding scope. This skill adds the commit SOP and the message format — nothing else. No test gate, no review pass: the architect runs those when they want them, they are not part of committing.

## Current Changes

!`git changes`

## Full Diff

!`git diff HEAD`

## Recent Commits

!`git log --oneline -10`

## SOP

1. **Stage.** Stage all changes unless the architect named a subset. Nothing to commit → say "Nothing to commit." and stop. Sanity check before committing: secrets, credentials, unrelated files, anything that does not belong → warn and confirm.
2. **Commit.** Write the message in the format below — match the style of the recent commits above. Commit with it. The architect can amend after: `git commit --amend`.
3. **Verify.** Exit code 0, then `trace status` shows a clean tree. Report "Committed: <sha> <subject>".
4. **Session notes.** After the commit, review session notes and suggest which should become permanent — in global/project Claude.md, skills, agents, rules, or commands as appropriate. Present suggestions only; do not act on them.

## Format

```
<type>: <subject - what changed, all changes summarized>

<what changed + why, combined naturally>

<additional context if multi-file or complex>:
- <change 1>
- <change 2>

<file tree>
├── path/to/modified.ts*   <- brief annotation
└── path/to/context.ts
```

## Anatomy

```
feat: add auto-migrations to deploy pipeline

Migrations now run automatically on every deploy via Trellis hook.
Safe because symlink switch happens AFTER migrations succeed.

deploy/
├── hooks/build-after.yml*    <- run migrations post-deploy
├── hooks/deploy-prepare.yml
└── docs/migrations.md*       <- design rules added
```

## Rules

1. **Type prefix:** `feat`, `fix`, `chore`, `refactor`, `docs`, `test`
2. **Subject:** lowercase after colon, <72 chars, summarizes all changes
3. **Body:** what+why woven together (not separate sections)
4. **File tree:** at end, show modified (*) and relevant context files
5. **No self-reference:** never "I", "we", "Claude"
6. **Bullets:** for multi-concern commits, group by area

## Examples

### Single-concern fix

```
fix: prevent cron ping pileup when requests take longer than interval

WordPress wp-cron.php uses ignore_user_abort(true), so PHP keeps
processing after client timeout. With 10s interval and 5s timeout,
requests piled up. Now skips ping if previous request is in flight.

app/
├── Services/CronPing.php*    <- added in-flight check
└── config/schedule.php       <- interval config lives here
```

### Multi-concern feature

```
feat: add Matomo configurator, 1Password secrets, and security hardening

Matomo:
- MatomoConfigurator with MaxMind GeoIP download
- Patches to remove newsletter and update nags

Secrets:
- 1Password integration via .vault_pass
- `bun secrets` command for local env vars

app/
├── Configurators/
│   └── MatomoConfigurator.php*   <- new configurator
├── Commands/SecretsCommand.php*  <- bun secrets
├── .vault_pass*                  <- 1Password integration
└── trellis/
    └── group_vars/all/vault.yml* <- encrypted secrets
```

### Minimal (trivial changes)

```
chore: update aws-sdk-php to fix security advisory

composer.lock*
```

## Anti-patterns

- `Completely turned off cors` → no type, no why
- `Fixed stuff` → vague
- `I added the feature` → self-reference
- No file tree for multi-file changes

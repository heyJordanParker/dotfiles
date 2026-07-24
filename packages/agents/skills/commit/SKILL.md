---
name: commit
description: |
  Mandatory contract for every commit. Loads automatically when the classifier authorizes a commit — it injects "Skills to execute: /commit" whenever the Architect asks to commit ("/commit", "commit this", "create a commit"). Holds the whole commit job: stage, write the message, commit, verify, then suggest which session notes should become permanent — plus the commit-message format (type prefix, what+why body, file tree). TRIGGER on every commit-authorized turn, or when the Architect asks to write or revise a commit message. DO NOT TRIGGER when the Architect has not asked to commit — applying changes, deploying, shipping, or replacing files are not commit requests.
---

# Commit

- The Architect approved committing the work.
- The `cto` Prompt governs reading before claiming, proving it ran, and holding scope.
- This Skill adds the commit Process and the commit-message shape.
- Tests and Review are separate; the Architect runs them when wanted.

## 1. Load the repository state

Current changes:
!`git changes`

Full diff:
!`git diff HEAD`

Recent commits:
!`git log --oneline -10`

## 2. Stage the requested changes

Stage all changes unless the Architect named a subset.

IF no changes are available to commit:
### Stop with the exact nothing-to-commit message
Say `Nothing to commit.` and stop.

IF current changes include secrets, credentials, or unrelated files:
### Warn and confirm before staging
Name the files that do not belong, then wait for the Architect before staging or committing.

## 3. Write and commit the message

Write the message in the shape below, match recent commit style, then commit with it. The Architect can amend after with `git commit --amend`.

### Describe the staged changeset, not your own work
Other Agents work the same branch, so the staged diff is larger than your Context. Write the message from the step-1 diff, covering every staged change equally. Run `trace diff` and read any staged change you do not recognize before writing a word about it.
Never: a subject or body scoped to the changes you made this session while the diff carries more.

### Use the repository commit-message shape
The type prefix is one of `feat`, `fix`, `chore`, `refactor`, `docs`, or `test`. The subject is lowercase after the colon, under 72 characters, and summarizes every committed change. The body weaves WHAT changed and WHY together instead of splitting them into separate sections. The file tree comes last and marks modified files with `*` beside relevant context files.

Template:
  ```
  <type>: <subject - WHAT changed, all changes summarized>

  <WHAT changed + WHY, combined naturally>

  <additional detail if multi-file or complex>:
  - <change 1>
  - <change 2>

  <file tree>
  ├── path/to/modified.ts*   <- brief annotation
  └── path/to/context.ts
  ```

### Write the commit message without self-reference
The message names the change, not the Agent that made it.
Never: `I added the feature`, `we fixed it`, or `Claude updated the files`.

### Group multi-concern commits by area
Use bullets only when a commit has more than one concern.

Example: single-concern fix.
  ```
  fix: prevent cron ping pileup when requests take longer than interval

  WordPress wp-cron.php uses ignore_user_abort(true), so PHP keeps
  processing after client timeout. With 10s interval and 5s timeout,
  requests piled up. Now skips ping if previous request is in flight.

  app/
  ├── Services/CronPing.php*    <- added in-flight check
  └── config/schedule.php       <- interval config lives here
  ```

Example: multi-concern feature.
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

Example: trivial change.
  ```
  chore: update aws-sdk-php to fix security advisory

  composer.lock*
  ```

Never: `Completely turned off cors`, `Fixed stuff`, or a multi-file commit without a file tree.

## 4. Verify the commit

The commit command must exit 0, then `trace status` must show a clean tree. Report `Committed: <sha> <subject>`.

## 5. Suggest permanent session notes

After committing, read session notes and suggest which should become permanent in global or project Claude.md, Skills, Agents, Rules, or Commands. Suggest only; do not act.

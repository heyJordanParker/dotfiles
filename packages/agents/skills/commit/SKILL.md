---
name: commit
description: |
  Mandatory contract for every commit. The Architect authorizes a commit by typing /commit, for that turn only, and the classifier then injects "Skills to execute: /commit". Holds the whole commit job: commit your own changes, write the message, commit, and verify, plus the commit-message format (type prefix, Capability subject, Problem and Solution body, file tree). TRIGGER on every turn where the Architect typed /commit, or when the Architect asks to write or revise a commit message. DO NOT TRIGGER on any other turn: "commit this", applying changes, deploying, shipping, and replacing files authorize nothing.
---

# Commit

- The Architect approved committing the work.
- This Skill adds the commit Process and the commit-message shape.

## 1. Load the repository state

Current changes:
!`git changes`

Full diff:
!`git diff HEAD`

Recent commits:
!`git log --oneline -10`

## 2. Commit only your own changes

- Other Agents work the same tree in parallel.

IF you changed nothing:
### Stop with the exact nothing-to-commit message
Say `Nothing to commit.` and stop.

IF your changes hold a secret or a credential:
### Stop and warn the Architect
Commit nothing. Show the Architect the secret's file and line, then wait for the Architect's answer.

## 3. Write and commit the message

Make one commit. Write the message to a file, then commit your files with `git commit -F <message-file> -- <your files>`.

### Describe everything the commit records, with equal weight
Your files can hold another Agent's edits, and the commit takes each file whole. The changes you worked on last fill most of your Context, so weigh each change by its effect on the User, never by how much of it you remember.

### Write WHAT changed and WHY as Capabilities, Problems, and Solutions
The type prefix is one of `feat`, `fix`, `chore`, `refactor`, `docs`, or `test`. The subject is lowercase after the colon, under 72 characters, and says what the User can do now, covering every committed change. The body says WHAT changed and WHY: the Problem each change solves and the Solution it takes. The body uses the product's words, and the file tree holds the code's names. The file tree comes last, drawn with /show-me but without the `+` and `-` markers, because a commit message shows no color. Each annotation says WHAT that file now does. An added or deleted file's annotation opens with `added` or `deleted`, then says why.

Template:
  ```
  <type>: <what the User can do now, largest User impact first>

  <the Problem the commit solves and the Solution it takes>

  <what the User can do now>:
  - <WHAT changed and WHY>

  <annotated file tree from /show-me>
  ```

### Order Capabilities by User impact
The Capability with the largest effect on the User comes first, in the subject and in the body.

### Head each group with the Capability it gives
Use one bullet group per Capability when the commit gives more than one. The heading says what the User can do now, as in `Track visitors with Matomo:`.

### Write the commit message without self-reference or Claude attribution
The message names the change, not the Agent that made it, and ends at the file tree.
Never: `I added the feature`, `we fixed it`, `Claude updated the files`, a `Claude-Session:` trailer, or a `Co-Authored-By: Claude` line.

Example: one Capability.
  ```
  fix: keep cron running when a request outlasts the ping interval

  WordPress wp-cron.php keeps running after the client times out, so with a
  10s interval and a 5s timeout the requests piled up and the site stalled.
  The ping now skips while the previous request is still in flight.

  app/
  ├── Services/CronPing.php*    <- skips a ping while one is in flight
  └── config/schedule.php       <- the interval the ping runs on
  ```

Example: several Capabilities.
  ```
  feat: track visitors with Matomo and load secrets from 1Password

  Sites had no analytics, and every secret lived in plain files on the
  server. Matomo now installs configured, and secrets load from 1Password.

  Track visitors with Matomo:
  - Matomo installs with GeoIP, so visits resolve to a country
  - The newsletter and update nags are gone from its dashboard

  Load secrets from 1Password:
  - Deploys read secrets from the 1Password vault
  - `bun secrets` loads the same secrets for local development

  app/
  ├── Configurators/
  │   └── MatomoConfigurator.php*   <- added, installs Matomo with GeoIP
  ├── Commands/SecretsCommand.php*  <- added, runs bun secrets
  ├── .vault_pass*                  <- added, connects deploys to 1Password
  ├── .env.production*              <- deleted, 1Password holds these secrets now
  └── trellis/
      └── group_vars/all/vault.yml* <- the encrypted secrets
  ```

Example: trivial change.
  ```
  chore: close the aws-sdk-php security advisory

  composer.lock*
  ```

Never: `Completely turned off cors`, `Fixed stuff`, or a multi-file commit without a file tree.

## 4. Verify the commit

### Report the commit once the command exits 0
Report `Committed: <sha> <subject>`.

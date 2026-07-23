# Update the Dent skill

- A stale skill can carry wrong API instructions, so update before Dent writes.
- The Agent owns the update when a local harness can run commands; the User should not touch a terminal.
## 1. Read the staleness signal

The CLI variant surfaces staleness through the first `dent check` in the Dent Process.

- IF `dent` is not on PATH, run each Dent CLI command as `npx @parkerlabs/dent@latest <command>`.

### Treat either stale message as a stop-before-write condition
Pause Dent writes, tell the user the skill is stale, and update it before touching data.
Example: `Dent skill update available: installed v0.1.0, npm v0.1.1.`
Never: continue a write after the session-start check or `dent check` says an update exists.

## 2. Refresh the installed local skill

### Use the package updater
Run the updater from the package; it detects the configured Claude Code and Codex harnesses and rewrites the installed skill tree.
Template:
  ```bash
  dent update
  ```

### Do not hand the terminal step back to the user
Agents with shell access run the update themselves and report what changed.
Never: "Please run the update command and tell me when it finishes."

## 3. Verify the update landed

### Check the installed skill after the update
Run the package check and continue only when it reports the skill is current.
Template:
  ```bash
  dent check
  ```

Example: `Dent skill is up to date (v0.1.1).`
Never: assume the update worked without a successful check.

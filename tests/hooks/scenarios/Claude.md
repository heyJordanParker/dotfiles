# WHY

Saved real gate-misfire moments for manually replaying Hook gates against the conversation context that produced the bad block or miss.

# Facts

- This folder stores manual replay scenarios for Hook gates.
- `replay.py` runs one scenario JavaScript Object Notation file against the real gate.
- Scenario files carry `gate`, `state`, `transcript`, and `note`.
- `transcript` stores real Claude Code transcript records.
- The last assistant message in `transcript` is the reply the gate judges.
- `babysitter.*.json` files replay the `babysitter` gate.
- `completion.*.json` files replay the `validate_completion` gate.

# WHY

Saved real gate-misfire moments for manually replaying Hook gates against the conversation context that produced the bad block or miss.

# Facts

- This folder stores manual replay scenarios for Hook gates.
- `replay.py` runs one scenario JavaScript Object Notation file against the real gate.
- Scenario files carry `gate`, `state`, `transcript`, and `note`.
- `transcript` stores real Claude Code transcript records.
- The last assistant message in `transcript` is the reply the gate judges.
- `babysitter` is the one Stop gate, so every scenario names it.
- `babysitter.*.json` files hold message-quality moments and `completion.*.json` files hold work-integrity ones, from when two gates split that judgement.
- `state` carries a live value, `propose` or `execute`; a retired spelling reads as `propose` and silently drops the Rules that need `execute`.

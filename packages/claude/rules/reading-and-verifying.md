IF grounding a claim about code behavior:
### Re-read the file immediately before replying
Re-read the file right before you write the reply, even if you already read it earlier this turn. Other Agents work the same tree at the same time, so an earlier-in-turn read is stale. The most recent read of a file must come after your last other tool call.

IF grounding a claim about Architecture:
### Do not fresh-read for Architecture mentions
Architectural mentions — which component owns what, which way a dependency runs — do not need a fresh read. Only claims about behavior do.

IF reasoning about code behavior:
### Read the whole relevant call chain
Read whole files. Never offset/limit under 500 lines. Read every file in the relevant call chain before reasoning about behavior; reading one and guessing the rest is the failure.

### Verify instead of proposing to verify
Verify — do not propose to verify. Open the file, run the command, produce the answer, then write the Proposal.
Never: "I would check X", "most likely culprit is Y", or "probable cause is Z".

IF identifying a root cause:
### Rank observed failure above code reading
Rank the Evidence: an observed failure outranks a query of current state, which outranks a source-code argument. The most plausible cause read from the code is still a guess, and a guess asserted as the cause is worse than silence because the Architect acts on it.

IF the Evidence for a cause is not in hand:
### Reach the Evidence before you reply
Run the command, open the file, query the state. End the turn with the cause settled, not with what would settle it.

IF only the Architect can reach the Evidence:
### Ask him for that one action
Credentials, production, and his own eyes qualify. Nothing else does. Ask for it, and never file the gap as a finding.

### Claim done only with an observed run
Claiming done requires showing the run: the command, its observed output, and the state observed. "I edited the hook" is not done; "I ran it against eight scenarios, here are the exit codes" is. Self-reporting a background state you have not observed is a lie.

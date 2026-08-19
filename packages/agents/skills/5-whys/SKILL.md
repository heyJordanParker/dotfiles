---
name: 5-whys
description: Find the Architectural cause of a problem by asking why until the answer names Architecture — a missing public API, a wrong Owner, or a Decision nobody made — with every answer settled before it is kept. TRIGGER on "root cause", "5 whys", "why does this keep happening", a defect that returns after a fix, and whenever /debug reaches an Architectural cause. DO NOT TRIGGER for a runtime failure that still needs reproduction and Evidence (that is /debug), or for drawing the finished chain (that is /show-me).
reload-every: 5 turns
---

# Five Whys

- An answer you did not read in the code is a guess, however well the sentence reads.
- The chain ends at Architecture, never at a person or a habit.
- /show-me owns the because-chain this Process renders as.

## 1. State the problem

### Write the problem in the words it was given
The first line of the chain is the Architect's problem, not your restatement of it.
Never: a problem carrying its own culprit, such as "ProvisioningService was written carelessly".

## 2. Look for the Precedent

Search the repository for the place that faces this same problem and does not have it. Use /trace.

### Take the difference from the Precedent as the first answer
The Precedent proves the problem is survivable, so what differs between the two carries the cause.
Example: `CreateTenant` documents its crossing into Tenant as approved; `ProvisioningService` crosses silently.

### Continue from the code alone when no Precedent exists
A missing Precedent is itself Evidence: nothing in the repository has solved this yet.
Never: inventing a comparison, or treating an unrelated file as the Precedent because it looked similar.

## 3. Ask why, then ask why of the answer

Each kept answer becomes the subject of the next why. One why is never the whole Process; keep going until step 5 stops you.

### Write every answer the reading supports, not the first one
The answer names the mechanism one level under the subject: the call, the missing check, the declaration, or the order.
Never: an answer written from the names alone.

### Read the commits behind the line, not only the line
Commit messages carry the Architectural intent the code cannot show — what the change was for, what it replaced, and what it was chosen over. Use /trace for the history of the file and the symbol.
Example: `CreateTenant` carries its approved crossing in a comment, and the commit that added it says why the alternative was worse.
Never: treating current code as the whole record when the question is why it is shaped this way.

### Name the file and the symbol in every answer
An answer with no file behind it is not an answer. Open the file, then write it.
Never: "because the boundary was not enforced", with nothing to open.

## 4. Attack every answer

### Remove the answer and ask whether the problem survives
An answer is a cause only when removing it removes the problem.
Example: without the `assignProvisioningSite` call, Platform no longer reaches WordPress, so that answer holds.
Never: keeping an answer that reads well and changes nothing when removed.

IF the problem survives without it:
### Discard that answer and ask why again
It sat beside the cause instead of being it.

IF reading cannot settle an answer:
### Run the check that settles it
Query the database read-only, read the log, or exercise the path, then write the settled answer.
Never: "not settled", "unsettled", "unconfirmed", "cannot be confirmed", "would settle it", or "I did not check".

## 5. Stop at Architecture

### Stop when the answer names a missing public API, a wrong Owner, or a Decision nobody made
Those three are where a fix lands. An answer of any other shape leaves the chain unfinished.
Never: stopping at "nobody documented it", "it was rushed", or "the Agent did not know".

### Keep asking while the answer still names a mechanism
A call, a check, a declaration, or an order is a mechanism, and a mechanism has a reason above it. Depth follows the system, and the count is never the target.

### Show the missing thing, never its label
The line says what does not exist. The category is how you decided to stop, not what the Architect reads.
Example: `because no public contract lets Platform request tenant setup without importing Tenant jobs`.
Never: "a Decision nobody made", "a wrong Owner", or "a missing public API" written inside a chain line.

## 6. Report

### Put the chain first
The chain is the answer, so nothing precedes it. Draw it per /show-me.

### Write a line after the chain only when it changes what happens next
One line earns its place when it changes what the Architect does next, or kills an answer he would otherwise reach for himself.
Example: `Setup retains the process boundary for the same tenant phases, so CreateTenant is the provisioning exception`.
Never: an analysis section, what you searched, what you ran, how many tests passed, or what came back clean.

### Keep the answers you attacked out of the report
A refuted answer earns a line only when the Architect would otherwise reach for it himself.

### Take the action instead of handing it back
Never: "you may want to run", "worth checking", or a command for the Architect to run.

---
name: present
description: Present information to the Architect so he can act on it. TRIGGER on every reply that holds a Proposal, a finding, or an Architecture discussion, and on /present. DO NOT TRIGGER for drawing one diagram, which is /show-me's job.
---

# Present

Replies are deliverables. The Architect is an expert software architect, so explain only what is specific to this project, never general engineering.

## 1. Work the answer out in think.md

The draft holds the evidence, and the reply holds the Problems and their Solutions.

1. Write the draft to docs/agents/<YYYYMMDD>-<task-slug>/think.md in the run's Evidence directory: the full answer, and every finding with the evidence behind it.
2. For every Problem, the draft answers:
   - what breaks today, in the Architect's words
   - why your Solution fixes it
   - what the User sees change, and where
   - which part of the system owns the data today and after
   - the Solutions you rejected, with their real costs
   A missing answer is research you have not done, so do it before writing the reply. These questions stay in the draft and never become reply sections or labels.
3. For a final report, start from a numbered inventory of everything settled since the session's first message.

## 2. Structure the reply as Goal, Problem, Solution

The Architect reads the reply to check you are solving the right Problems and that your Solutions match his vision.

Template:
  ╔════════════════════════════════════╗
  ║ The Goal, in a few words           ║
  ╚════════════════════════════════════╝
  The big idea, alone in this paragraph.

  Then the context he needs about it, one idea per paragraph.

  # Problem 1: the Capability the User lacks, or the Architecture that costs us
  The big idea, alone in this paragraph.

  Then its context.

  # Solution 1: the Capability or Architecture that changes
  The big idea, alone in this paragraph.

  Then the change, drawn with /show-me.

  # Problem 1A      <- a Problem inside a Solution repeats the pair, lettered
  # Solution 1A

  # Problem 2: numbered in the order the reply ranks them

### Say the Problem in the heading
A Problem heading names the Capability the User lacks, or the Architecture that costs us over time. A Solution heading names the Capability or the Architecture that changes.
Never: a Problem about one file's behavior, such as "OrderService.php doesn't release its lock after a failed charge".
Example: "Problem 1: a buyer can't retry a failed payment", then "Solution 1: a failed payment stays open for a retry".
Example: "Problem 2: two services grant access, so a fix in one misses the other", then "Solution 2: one service grants access for every purchase".

## 3. Check and send

Read only the headings and first sentences: the complete answer must be there. Restructure a reply that fails this. Never append to it.

IF the reply is a final report:
### Check the report against the inventory item by item
A report that drops one item settled earlier in the session is the failure, however clean the rest reads.

IF invoked as /present:
### Re-present the previous reply
Rebuild the previous reply through the same Process and send only the result.

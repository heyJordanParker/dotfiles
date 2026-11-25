---
name: present
description: Structure every reply that holds a Proposal, a report, or an Architecture discussion, so the Architect understands it by scanning its headings, first sentences, drawings, and callouts. TRIGGER on those replies and on /present. DO NOT TRIGGER for drawing one diagram, which is /show-me's job.
---

# Present

- The Architect scans a reply from heading to heading, and must grasp its Architecture without reading it word for word.
- This Skill owns the reply's structure. communication.md owns its sentences, and /show-me owns its drawings.
- The Architect is an expert software architect, so explain only what is specific to this project, never general engineering.

## 1. Pick the structure from what the Architect asked for

IF the Architect asked a question:
### Give the answer first and alone, then what proves it
Draw the data, the code, or the Architecture that proves it with /show-me.
Never: Problems and Solutions the question did not ask for.

IF the reply reports progress or finished work:
### Report the results as exact numbers, and what changed since the Architect's last message
Never: a progress word such as "building" in place of a number, or an explanation of the numbers beside them.

IF the reply proposes a change:
### Structure it as Goal, Problems, and Solutions
The Architect reads a Proposal to check you are solving the right Problems and that your Solutions match the Architect's vision.
Template:
  ╔════════════════════════════════════╗
  ║ The Goal, in a few words           ║
  ╚════════════════════════════════════╝
  The big idea, alone in this paragraph.

  # Problem 1: the Capability the User lacks, or the Architecture that costs us
  The big idea, alone in this paragraph.

  Then its context, one idea per paragraph.

  # Solution 1: the Capability or Architecture that changes
  The big idea, alone in this paragraph.

  One sentence stating an idea, then its /show-me drawing.

  A comment on one part of the drawing, in its own paragraph.

  # Problem 1A      <- a Problem inside a Solution repeats the pair, lettered
  # Solution 1A

  # Problem 2: numbered in the order the reply ranks them

## 2. Shape each Problem and Solution

### Say the Problem in the heading
A Problem heading names the Capability the User lacks, or the Architecture that costs us over time. A Solution heading names the Capability or the Architecture that changes.
Never: a Problem about one file's behavior, such as "OrderService.php doesn't release its lock after a failed charge".
Example: "Problem 1: a buyer can't retry a failed payment", then "Solution 1: a failed payment stays open for a retry".
Example: "Problem 2: two services grant access, so a fix in one misses the other", then "Solution 2: one service grants access for every purchase".

### Show the whole Solution you would ship
Draw every change the Solution makes, so the Architect can approve or correct it as it stands.
Never: a summary of the Solution's parts, a list of decisions for the Architect to make, or options when only one would ship.

### Give each level a distinct style
The Architect tells the levels apart by their look while scanning. The Goal is a double-line box in a `text` code block. Every Problem and Solution is a `#` heading. A nested pair is lettered.

### Order a Solution's drawings from the User inward
Draw with /show-me the Capabilities with their WHY first, then the UX, then each Module the Solution changes, then the Architecture and code. The Architect judges what the User gets before how it is built.
Never: a section per file. The Architect designs Capabilities and Modules, and a Module's drawing names its files.

### Put what the Architect must not miss in a callout
A Decision the Architect owns, a change to a system the request did not name, or a rewrite of a Module gets a callout where it applies, never a sentence inside another paragraph.
Example: `> **Decision for you:** the fix also changes how Orders stores refunds.`

## 3. Check and send

Read only the headings, first sentences, drawings, and callouts. The complete answer must be in them. Restructure a reply that fails this, and never append to it.

IF the reply is a final report:
### Check it against everything settled since the session's first message
A report that drops one item settled earlier in the session is the failure, however clean the rest reads.

IF invoked as /present:
### Re-present the previous reply
Rebuild the previous reply through this Process and send only the result.

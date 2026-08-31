---
name: discover
description: Find every option that solves a problem and return them ranked per /pcc. TRIGGER on "options", "approaches", "what are the ways", "how could we", and before proposing an Architecture Decision. DO NOT TRIGGER to rank options already found; use /pcc.
---

# Discover

## 1. Write every option

### Write every option as one line before you understand any
An option stays one line until every option is written. Sources: the Precedent in this repo (/trace), the shape an established system uses for this problem (/research), and the Architect's words.

## 2. Understand every option

### Give every option the same depth
For each option, trace the files it touches, the callers it changes, and the Precedent it builds on. For an external option, read a dated source. Find its failures per /architecture step 2.

## 3. Rank per /pcc

### Rank every option from step 2 per /pcc
No option is left out before /pcc runs.

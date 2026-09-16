---
name: discover
description: Find every option that solves a problem, so the one you propose is the best one and not the first one you thought of. TRIGGER before every Architecture Decision, and on "options", "approaches", "what are the ways", "how could we". DO NOT TRIGGER to score options already found; use /rank.
---

# Discover

## 1. Write every option

### Write every option as one line before you understand any
An option stays one line until every option is written. Sources: the Precedent in this repo (/trace), the shape an established system uses for this problem (/research), and the Architect's words.

## 2. Understand every option

### Give every option the same depth
For each option, trace the files it touches, the callers it changes, and the Precedent it builds on. For an external option, read a dated source. Use /architecture step 2 to find its failures.

## 3. Rank and pick

### Rank every option from step 2, then propose one
Use /rank on every option, then propose the one you would ship.

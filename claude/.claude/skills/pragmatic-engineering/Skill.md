---
name: pragmatic-engineering
description: Use when planning features, reviewing code, or making architectural decisions. Applies principles from Musk, Valve, Spotify, Shopify, 37 Signals to ensure pragmatic, ship-focused development.
---

# Pragmatic Engineering

Ship fast, ship often, ship simple.

**Core principle:** The only metric that matters is shipping. Delete more than you create. Optimize for learning velocity.

## The Rule

Question everything. Delete aggressively. Simplify ruthlessly. Only then optimize.

## The 5-Step Algorithm

Apply in order. Never skip ahead.

1. **Question requirements** – All requirements are dumb until proven otherwise. Question AI-generated requirements especially hard.
2. **Delete** – Delete aggressively. If you're not occasionally adding things back, you're not deleting enough.
3. **Simplify** – Complexity slows everything. Never optimize something that shouldn't exist.
4. **Speed up** – Only accelerate after steps 1-3. Most time savings are in deletion, not acceleration.
5. **Automate** – Automate last, not first. Common mistake: automating a process that shouldn't exist.

## Shipping Philosophy

### Ship to Learn

- **TOMASP** – The Only Metric A Startup Pursues: Ship
- **2-day quality horizon** – Never more than 2 days from shippable
- **Launch, tweak, improve** – Ship to learn, not to perfect
- **Expose early** – Show work early, not after months of isolation
- **Velocity matters** – Fail fast, learn fast, improve fast

### Innovation Over Predictability

- **Freedom to fail** – Failure drives innovation
- **Try new things** – Optimize for experimentation
- **Programmer happiness** – Joy of use matters

## Code Philosophy

### Simplicity & Elegance

- **Maintenance mode** – Code fails in maintenance, not creation
- **Small files** – Strict encapsulation, one-directional dependencies
- **10-minute rewrite rule** – If a component takes longer to rewrite, it's too complex
- **No one-shot code** – Never write code for "just this once." If it's worth writing, write it properly

### Loosely Coupled, Tightly Aligned

- **Independent components** – One-way dependencies
- **Shared contracts** – Interfaces, not implementations
- **YAGNI** – Abstract after duplication, not before. Abstractions emerge from 3+ duplications

### Build Less

- **Minimize code** – Intentionally build and maintain as little as possible
- **Reuse libraries** – Never rewrite what exists
- **Read docs first** – Understand before using
- **3rd party for heavy lifting** – Use reliable, well-maintained external code

## Decision Making

### Epicenter First

- **What can't be removed?** – That's your core
- **Interface first** – Start with user experience, build backward
- **Appetite over estimates** – "How much is this worth?" not "How long will it take?"

### Convention Over Configuration

- **Strong defaults** – Less decisions
- **No assumptions** – Pattern matching isn't enough. Read the code
- **Developer experience > performance** – Except when proven otherwise

## Working Style

### Iterate Over Innovate

- **Stick with approach** – Until told to change
- **Suggestions after** – Not during current iteration
- **Test everything** – Untested code is a guess
- **Verify before claiming** – Changes work before claiming completion

### Good Not Nice

- **Don't be sycophantic** – Correct me when wrong
- **Software > feelings** – Never say "You're absolutely right!" before reading code
- **Report failures immediately** – Don't work around silently
- **Say when stuck** – "I'm stuck because X. Should I Y or Z?"

## Documentation

### Zero Prompting

- **No detailed prompts** – If it requires explanation, docs are lacking
- **Self-documenting** – Code and structure tell the story
- **Context over comments** – Naming and organization replace prose

## Red Flags

Stop if you see:

- Building before understanding library behavior
- Creating abstractions "for later"
- Duplicating 3rd party functionality
- Hiding errors or limitations
- Assuming intent without asking
- Claiming something works before testing
- Automating before simplifying
- Optimizing before deleting
- Adding features before questioning requirements
- Code that takes >10 minutes to rewrite
- Components with bi-directional dependencies
- Abstractions with <3 use cases

## Process

1. **Question** – Does this need to exist?
2. **Delete** – What can be removed?
3. **Simplify** – What remains, make simple
4. **Ship** – Get it in front of users
5. **Learn** – Iterate based on reality, not assumptions

---
name: block-domain-words-for-code
description: Flags Domain.md words used to name code.
context: fork
model: haiku
effort: low
allowed-tools: Read
paths:
  - "**/CLAUDE.md"
  - "**/Claude.md"
  - "**/rules/**/*.md"
  - "**/agents/**/*.md"
  - "**/skills/**/*.md"
metadata:
  level: error
  runs: edit
---

Domain.md is in your context. Its words, such as Slice, Precedent, Capability, Module, and Problem Hierarchy, name the business and the work. They never name code: a file, a folder, a class, a method, a variable, or a command.

Report each sentence that uses a Domain.md word to name code, such as "the Slice moves the Precedent into the Problem Hierarchy" said about files and methods. Its message names the word and what the sentence means by it, such as: "Slice" names a commit here. Domain.md words name the business, never code.

Only a capitalized Domain.md word counts, such as Slice or Process. The same word in lowercase, such as "a shell command" or "a child process", is plain English.

Leave alone:
- a sentence that uses a Domain.md word for what Domain.md defines, including the file or folder that records it, such as the Prompt Architecture document or the Decisions folder
- a Domain.md word that names a part of Claude Code itself, such as an Agent, a Skill, a Rule, a Hook, a Command, or a Prompt, because Domain.md defines those parts
- a word inside backticks, which names code by its own name

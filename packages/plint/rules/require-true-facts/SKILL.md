---
name: require-true-facts
description: Flags each Claude.md Fact the code contradicts.
context: fork
model: sonnet
allowed-tools: Read, Grep, Glob
paths:
  - "**/CLAUDE.md"
  - "**/Claude.md"
metadata:
  level: error
  runs: manual
---

A Fact is a declarative statement about the code, such as a file and what it holds, a shell command and what it does, or where a setting lives. Read the code, folders, and files each Fact names, from the project root.

Report each Fact the code contradicts, with what the code shows instead. Leave alone a Fact the code confirms, and a Fact about something outside the repository that you cannot read.

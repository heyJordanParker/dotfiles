---
paths:
  - "packages/tmux/**"
  - "packages/zsh/**"
  - "packages/ghostty/**"
  - "packages/karabiner/**"
---

### Preserve macOS terminal defaults
Never bind custom keybindings over the macOS terminal defaults: the readline Ctrl set, the Alt/Option word-movement set, and the Cmd clipboard set. They are system-wide, and overriding them breaks expected shell behavior.

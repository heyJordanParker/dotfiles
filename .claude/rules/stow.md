---
paths:
  - "packages/**"
  - "scripts/stow.py"
  - "setup.sh"
---

### Keep package contents at their target shape
Every package contains exactly the files that land at its target.
Never: inner mirror wrappers such as `.config/<tool>/` or `.<tool>/`.

### Keep packages self-contained
Each package owns its files, configs, and ignores. A package ignore lives as a nested `.gitignore` inside the package, never as an entry in the root one, so deleting the package cleans everything.

IF adding, removing, or renaming a file or directory inside a package, or stowing a brand-new package:
### Restow the package
Restow that package with `cd <repo>/packages && stow -R -t <target> <pkg>`, or run `python3 scripts/sync.py` for the whole repo. A removed file leaves a dangling symlink at the target until stow cleans it.

IF adding a new package:
### Add the package target to `scripts/stow.py`
Put files at the package root, add the package → target entry in `scripts/stow.py` (`TARGETS`, or the `CONFIG` list for a `~/.config/<pkg>/` tool), then run `python3 scripts/sync.py`.

IF removing a package:
### Unstow before deleting the package
Run `stow -D -t <target> <pkg>` first, then delete the directory under `packages/`, then remove its line from `setup.sh`.

IF installing a command-line tool:
### Put the tool and config in their package homes
Add the tool to the Brewfile; config goes in its own package; a wrapper needing secrets or environment variables goes in `packages/bin/` and requires restowing `bin`. Python tools install via pipx.

### Keep `scripts/stow.py` as the package target source
Never break the per-package target mapping in `scripts/stow.py`. It is the single source; `setup.sh` and the pre-commit Hook restow through `scripts/sync.py`, which reads it.

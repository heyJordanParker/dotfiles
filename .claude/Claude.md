# Dotfiles

GNU Stow-managed dotfiles. Each subdirectory is a package.

## Packages

!`ls -d */ | grep -v '^\.'`

## After Editing

After modifying files in any package, restow it:
```bash
stow -R <package>   # e.g., stow -R zsh
```

## Adding New Packages

1. Create package directory: `mkdir <package>`
2. Mirror the target structure inside:
   ```
   <package>/
   └── .config/
       └── <app>/
           └── config.toml
   ```
3. Stow it: `stow <package>`

## Installing CLI Tools

When adding a new CLI tool:
1. Add to `Brewfile` (appropriate section)
2. If config needed: create package dir, add config files
3. If wrapper needed (secrets, env vars): add to `bin/.local/bin/`
4. Stow any new/modified packages: `stow -R <package>`

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

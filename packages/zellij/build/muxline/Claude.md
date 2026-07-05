# WHY

Purpose-built Zellij plugin that owns the tmux-parity tab-bar and status-bar behavior in one WebAssembly artifact.

# Facts

- `muxline` is a Rust Zellij plugin compiled to `wasm32-wasip1`.
- The installed artifact is `~/.config/zellij/plugins/muxline.wasm`.
- One `muxline.wasm` binary selects behavior through the `mode` plugin configuration key.
- The plugin modes are Indicator, RenameTab, RenameSession, and ReloadConfig.
- Indicator mode normalizes tab names and owns waiting and completed attention icons.
- RenameTab and RenameSession modes share the same line editor.
- ReloadConfig mode reads `~/.config/zellij/config.kdl` and hands the contents to `reconfigure`.
- `Cargo.toml` depends on `zellij-tile` version `0.44.1`.
- `setup.sh` builds the plugin with `cargo build --target wasm32-wasip1 --release`.
- `setup.sh` copies `target/wasm32-wasip1/release/muxline.wasm` into `~/.config/zellij/plugins/`.
- `packages/zellij/config.kdl` declares the `muxline` plugin.
- `packages/zellij/layouts/default.kdl` consumes the `pipe_cwd`, `pipe_cmd`, and `pipe_zoom` status segments.
- `packages/zsh/.zshrc` sends `muxline::cwd`, `muxline::cmd`, and `muxline::completed` pipe messages.

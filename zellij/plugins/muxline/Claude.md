# muxline
v1.2 | Updated: 2026-04-20

## Why

Out of the box, zellij does not provide several tab-bar and status-bar behaviors that tmux exposes by default and that day-to-day terminal-multiplexer use depends on:

- **Tab-name normalization.** Zellij surfaces every tab as `Tab #N` until the user renames it, and exposes the raw value to the status bar. Without normalization, the tab strip mixes `Tab #1` with `2: Dev` and reads inconsistently.
- **Per-pane attention indicators that auto-clear on focus.** Zellij has no concept of a pane signaling "I want attention" and clearing the signal once the user looks at it. Long-running commands that finish in a background tab leave no trace.
- **Pane-aware status segments (cwd / cmd / zoom) that update within one prompt-tick.** Zellij's internal `CwdChanged` polling interval (~1s) is slow enough that the status bar visibly lags the shell. There is no built-in `cmd` segment at all, and the zoom segment cannot be derived reliably from `PaneInfo` alone.
- **Full-readline rename modals for `prefix+r` / `prefix+R`.** Zellij's built-in rename UI does not pre-populate, does not support readline editing, and does not match the tmux `prefix+,` / `prefix+$` flow.
- **A reload-config keybind.** Zellij has no equivalent of tmux's `bind C source-file ~/.tmux.conf` — config changes require restarting the session.

Off-the-shelf zellij plugins each cover a slice of this gap and disagree about ownership of tab text, producing conflicts where two plugins fight over the same tab name. A single purpose-built plugin owns every tmux-parity behavior end-to-end, so tab-text ownership lives in one place.

## What

A single Rust → `wasm32-wasip1` zellij plugin compiled to one `muxline.wasm` artifact. The same binary runs in one of four modes, selected by the `mode` key in plugin configuration (`src/main.rs:60-86`):

- **Indicator** (default, invisible background) — loaded once at session start via `load_plugins`. Owns tab-name normalization, attention icons (⏳ / ✅), and the cwd / cmd / zoom pipe segments shipped to every zjstatus instance.
- **RenameTab** — on-demand floating modal launched by `prefix+r`. Pre-populates with the active tab's stripped name, full readline editing, submits via `rename_tab_with_id` then `close_self`.
- **RenameSession** — same modal, launched by `prefix+R`, submits via `rename_session`.
- **ReloadConfig** — invisible 1×1 floating instance launched by `prefix+C`. Reads `~/.config/zellij/config.kdl` off disk, hands it to `reconfigure`, closes itself. Mirrors tmux's `bind C source-file ~/.tmux.conf`.

### Requirements

- Must compile to a single `wasm32-wasip1` artifact — one wasm file, mode-dispatched at runtime
- Must be event-driven for all status-bar publishing — every input comes from event payloads (`PaneUpdate`, `TabUpdate`, `SessionUpdate`, `CwdChanged`, `Event::Key`, pipe messages)
- Must request only the permissions each mode needs — the permission vector in `load()` branches on mode (`src/main.rs:758-775`)
- Must auto-clear attention indicators on focus — switching to a tab containing a notified pane removes the icon
- Must broadcast pipe segments to every zjstatus instance — the layout's `default_tab_template` instantiates one zjstatus per tab, so `pipe_message_to_plugin` targets every discovered id

### Boundaries

- Never call synchronous shims that block the plugin worker — `get_pane_cwd`, `get_pane_running_command`, and similar block `plugin-exec-N` and cascade into `Action NewPane` SLA timeouts. All inputs come from event payloads or pipe messages
- Never depend on shell daemons or polling sidecars — the plugin replaces a previous shell-daemon polling design; new behaviors join this binary, not a parallel process
- Never split modes into separate binaries — one wasm artifact, one source file. Adding a mode means a new `Mode` variant and an arm in `update()`
- Never write to the real `/tmp` for status communication — communication is pipe-message-only; zjstatus's file-based `command_*` widget path was rejected because its `Event::RunCommandResult` handler doesn't set `should_render=true`
- Never target zjstatus by alias — `pipe_message_to_plugin`'s `with_plugin_url("zjstatus")` silently drops; always target by `destination_plugin_id` discovered from `SessionInfo.plugins`
- Never use `print_text_with_coordinates` for the rename modal — raw ANSI in `render()` keeps the cursor visible regardless of zellij's pane-cursor behavior
- Never read `PaneInfo.is_fullscreen` for the zoom segment — `Alt+z` toggles `TabInfo.is_fullscreen_active`; the pane-level field doesn't reliably reflect it. Subscribe to `TabUpdate` and publish on it, not just `PaneUpdate`

## How

### File Layout

```
muxline/
├── Cargo.toml          # zellij-tile dep + release profile (opt-level=z, lto, strip)
├── Cargo.lock
├── .gitignore          # /target/
└── src/
    └── main.rs         # all four modes, ~1100 lines
```

Single source file. Sections in order: mode + config parsing, `LineEditor`, `RenameState`, `ActivePaneState`, `State`, the `ZellijPlugin` impl (`load` / `update` / `render` / `pipe`), and a `scroll_for_cursor` helper.

### Build & Install

`setup.sh` (lines 79-88) handles everything on a fresh machine:

```sh
rustup target add wasm32-wasip1
(cd zellij/plugins/muxline && cargo build --target wasm32-wasip1 --release)
cp target/wasm32-wasip1/release/muxline.wasm ~/.config/zellij/plugins/
```

After local edits: rebuild and re-copy. There is no `stow` step — the wasm artifact is not stow-managed; `~/.config/zellij/plugins/muxline.wasm` is copied in by `setup.sh`. The other zellij configs (`config.kdl`, `layouts/`, `permissions.kdl`) are stowed.

### Permissions Model

Granted in `~/.config/zellij/permissions.kdl` (`zellij/.config/zellij/permissions.kdl:15-21`):

- `ReadApplicationState` — all modes (TabUpdate, PaneUpdate, SessionUpdate, CwdChanged)
- `ChangeApplicationState` — all modes (`rename_tab`, `rename_tab_with_id`, `rename_session`, `set_selectable`, `change_floating_panes_coordinates`, `rename_plugin_pane`, `close_self`)
- `ReadCliPipes` — all modes (the `zellij pipe --name "muxline::..."` message channel)
- `RunCommands` — ReloadConfig only (shells out to `cat ~/.config/zellij/config.kdl`)
- `MessageAndLaunchOtherPlugins` — Indicator only (`pipe_message_to_plugin` to zjstatus). Without it the shim silently no-ops — no error surfaces to the plugin, only a log entry on the server side

The plugin's `load()` (`src/main.rs:758-775`) requests only the subset its mode needs; the cache file pre-grants the union.

### Integration Points

- **`zellij/.config/zellij/config.kdl:42-48`** — declares the plugin under `plugins { muxline location="file:~/.config/zellij/plugins/muxline.wasm" }` and loads the Indicator instance via `load_plugins { muxline }`.
- **`zellij/.config/zellij/config.kdl:268-348`** — keybindings for the on-demand modes: `prefix+r` (RenameTab), `prefix+R` (RenameSession), `prefix+C` (ReloadConfig). All use `LaunchOrFocusPlugin` / `LaunchPlugin` with `mode "rename_tab"` / `"rename_session"` / `"reload_config"` and `skip_plugin_cache true`.
- **`zellij/.config/zellij/layouts/default.kdl`** — zjstatus is instantiated per tab via `default_tab_template`. The status bar consumes `{pipe_cwd}`, `{pipe_cmd}`, `{pipe_zoom}` widgets (lines 103-110) that the Indicator plugin pushes via `pipe_message_to_plugin`. Tab format `{index}{name}` (lines 79-84) consumes the normalized tab names.
- **`zsh/.zshrc:142-164`** — zsh `precmd` / `preexec` hooks fire `zellij pipe --name "muxline::cwd::$ZELLIJ_PANE_ID" --payload "$PWD"`, `muxline::cmd::$ZELLIJ_PANE_ID`, and `muxline::completed::$ZELLIJ_PANE_ID` on every prompt and command. The plugin's `pipe()` handler (`src/main.rs:1019-1106`) parses the `muxline::EVENT::PANE_ID` envelope. The pipe path exists because zellij's internal `CwdChanged` polling has a ~1s interval — too slow for tmux-parity feel; the precmd-pipe lands one prompt-tick later.

### Data Flow (Indicator mode)

```
zsh precmd/preexec        ──► zellij pipe ──► plugin.pipe() ──┐
zellij events                                                 │
  PaneUpdate / TabUpdate  ──► plugin.update() ────────────────┤
  SessionUpdate           ─── (discovers zjstatus plugin ids) │
  CwdChanged              ────────────────────────────────────┤
                                                              ▼
                                              State::publish_focused_pane()
                                                              │
                                          pipe_message_to_plugin
                                                              ▼
                                              zjstatus::pipe::cwd / cmd / zoom
                                                              │
                                                              ▼
                                              status bar render
```

Every status-bar update is change-detected in `publish_focused_pane` (`src/main.rs:432-505`) — only the segments whose displayed value changed are re-piped. zjstatus instances are discovered from `SessionInfo.plugins` by substring-matching `"zjstatus"` in the location, then targeted by `destination_plugin_id`.

### Tab Name Normalization

`State::normalize_base` (`src/main.rs:324-333`) shapes tab text so zjstatus's `{index}{name}` reads exactly like tmux:

- `Tab #N` (zellij default) → `""`, displays as `"1"`
- User-set `Dev` → `": Dev"`, displays as `"1: Dev"`
- Pane in tab triggers `waiting` pipe → name suffixed with ` ⏳`, displays as `"1: Dev ⏳"`

Reconciliation runs on every TabUpdate / PaneUpdate / pipe (`State::reconcile_tab_names`, `src/main.rs:580-622`) and re-issues `rename_tab` only when the desired text differs from the current text. The `pending_renames` set keeps the loop quiet during the round-trip.

### RenameTab / RenameSession Modal

- Plugin launches floating with `move_to_focused_tab true` and `skip_plugin_cache true`.
- On the first TabUpdate after permissions land, `size_self_from_active` (`src/main.rs:651-679`) reads the active tab's display area and centers a 60×3 floating pane.
- For RenameTab, the active tab's stripped name is captured into the `LineEditor` (`capture_tab_target`); for RenameSession, the current session name is captured from SessionUpdate (`capture_session_target`).
- `render()` (`src/main.rs:962-1016`) writes the input row as raw ANSI: clear the line, render the visible slice, render the cursor as an inverted-video cell. One `print!`, one `flush`, one host read per frame.
- `handle_key` (`src/main.rs:700-746`) maps Enter (submit + close), Esc (cancel + close), arrows / Home / End / Ctrl+A / Ctrl+E / Alt+B / Alt+F (movement), Backspace / Delete / Ctrl+D / Alt+D / Ctrl+W / Alt+Backspace / Ctrl+U / Ctrl+K (editing).

### ReloadConfig

`prefix+C` launches a 1×1 floating instance with `mode "reload_config"`. On permission grant, the plugin shells out (`run_command(["sh", "-c", "cat ~/.config/zellij/config.kdl"])`) — `sh -c` so `~` expands. The `RunCommandResult` handler feeds the contents to `reconfigure(contents, false)` (write-to-disk false, mirrors tmux's `source-file` semantics) and calls `close_self`. The plugin is invisible because `set_selectable(false)` runs before any prompt could appear.

## Ledger

- v1.0: One mode-dispatched wasm owns every tmux-parity tab-bar behavior, keeping tab-text ownership in one place
- v1.1: Pin zellij-tile landmines because re-discovering them costs hours
- v1.2: Reframed Why around zellij-vs-tmux behavioral gaps to keep project doc factual

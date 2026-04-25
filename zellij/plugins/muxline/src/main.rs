//! muxline — zellij plugin for custom tab-bar behavior.
//!
//! The same binary runs in two modes, selected by the `mode` key in plugin
//! configuration:
//!
//! ### Indicator mode (default, invisible background plugin)
//!
//! Loaded once at session start via `load_plugins { muxline }`.
//! Responsibilities:
//!
//! 1. **Tab name normalization** — make zjstatus tab display match tmux exactly:
//!    - Default `Tab #N` → empty string, so `{index}{name}` in zjstatus shows `"1"`.
//!    - User-set name "Dev" → `: Dev`, so `{index}{name}` shows `"1: Dev"`.
//!
//! 2. **Attention indicators** (replacement for shell-daemon polling):
//!    - Listens for `zellij pipe --name "muxline::waiting::$ZELLIJ_PANE_ID"` (or `completed::`).
//!    - Appends ⏳ or ✅ to the tab name containing that pane.
//!    - Auto-clears on focus — switching to the tab removes the icon.
//!    - Tab-level aggregation: any pane in the tab with a notification → tab shows the icon.
//!
//! ### RenameTab mode (on-demand interactive modal)
//!
//! Launched via keybinding:
//!
//!     bind "r" {
//!         LaunchOrFocusPlugin "muxline" {
//!             floating true
//!             move_to_focused_tab true
//!             skip_plugin_cache true
//!             mode "rename_tab"
//!         }
//!     }
//!
//! Normal input popup: visible text, full line-editor keybindings (arrows,
//! Home/End, Alt/Ctrl word motion, Alt+Backspace / Ctrl+W kill-word, Ctrl+U/K
//! kill-to-start/end, Delete, Backspace). Enter submits via
//! `rename_tab_with_id` + `close_self`. Esc cancels.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use zellij_tile::prelude::*;
use zellij_tile::shim::{
    change_floating_panes_coordinates, close_self, get_plugin_ids, pipe_message_to_plugin,
    reconfigure, rename_plugin_pane, rename_session, rename_tab, rename_tab_with_id,
    run_command, set_selectable, unblock_cli_pipe_input,
};

const USER_NAME_PREFIX: &str = ": ";

/// Target modal footprint — floating pane inner dimensions (chars × rows).
const MODAL_COLS: usize = 60;
const MODAL_ROWS: usize = 3;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum NotificationType {
    Waiting,
    Completed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Indicator,
    RenameTab,
    RenameSession,
    /// One-shot reload: reads ~/.config/zellij/config.kdl off disk and hands
    /// it to `reconfigure`, then closes itself. Invoked via prefix+C
    /// (mirrors tmux's `bind C source-file ~/.tmux.conf`).
    ReloadConfig,
}

impl Mode {
    fn from_configuration(config: &BTreeMap<String, String>) -> Self {
        match config.get("mode").map(String::as_str) {
            Some("rename_tab") => Mode::RenameTab,
            Some("rename_session") => Mode::RenameSession,
            Some("reload_config") => Mode::ReloadConfig,
            _ => Mode::Indicator,
        }
    }

    /// Rename-style modes share the same input UI, keybindings, and submit flow.
    fn is_rename(self) -> bool {
        matches!(self, Mode::RenameTab | Mode::RenameSession)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: bool,
    pub waiting_icon: String,
    pub completed_icon: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            waiting_icon: "⏳".to_string(),
            completed_icon: "✅".to_string(),
        }
    }
}

impl Config {
    fn from_configuration(config: &BTreeMap<String, String>) -> Self {
        let mut result = Self::default();
        if let Some(v) = config.get("enabled") {
            result.enabled = v == "true";
        }
        if let Some(v) = config.get("waiting_icon") {
            result.waiting_icon = v.clone();
        }
        if let Some(v) = config.get("completed_icon") {
            result.completed_icon = v.clone();
        }
        result
    }
}

/// Line editor for RenameTab mode.
#[derive(Default)]
struct LineEditor {
    buffer: Vec<char>,
    cursor: usize,
}

impl LineEditor {
    fn text(&self) -> String {
        self.buffer.iter().collect()
    }

    fn set(&mut self, s: &str) {
        self.buffer = s.chars().collect();
        self.cursor = self.buffer.len();
    }

    fn insert(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.buffer.remove(self.cursor - 1);
            self.cursor -= 1;
        }
    }

    fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
        }
    }

    fn home(&mut self) {
        self.cursor = 0;
    }

    fn end(&mut self) {
        self.cursor = self.buffer.len();
    }

    /// Previous word boundary: skip non-word chars, then skip word chars.
    fn prev_word(&self) -> usize {
        let mut i = self.cursor;
        while i > 0 && !is_word_char(self.buffer[i - 1]) {
            i -= 1;
        }
        while i > 0 && is_word_char(self.buffer[i - 1]) {
            i -= 1;
        }
        i
    }

    /// Next word boundary: skip word chars, then skip non-word chars.
    fn next_word(&self) -> usize {
        let len = self.buffer.len();
        let mut i = self.cursor;
        while i < len && is_word_char(self.buffer[i]) {
            i += 1;
        }
        while i < len && !is_word_char(self.buffer[i]) {
            i += 1;
        }
        i
    }

    fn move_word_left(&mut self) {
        self.cursor = self.prev_word();
    }

    fn move_word_right(&mut self) {
        self.cursor = self.next_word();
    }

    fn kill_word_backward(&mut self) {
        let start = self.prev_word();
        self.buffer.drain(start..self.cursor);
        self.cursor = start;
    }

    fn kill_word_forward(&mut self) {
        let end = self.next_word();
        self.buffer.drain(self.cursor..end);
    }

    fn kill_to_start(&mut self) {
        self.buffer.drain(..self.cursor);
        self.cursor = 0;
    }

    fn kill_to_end(&mut self) {
        self.buffer.truncate(self.cursor);
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[derive(Default)]
struct RenameState {
    target_tab_id: Option<usize>,
    editor: LineEditor,
    captured: bool,
    sized: bool,
}

/// Tracks the currently-focused pane + its derived display state so the
/// status bar reflects the RIGHT pane's cwd / cmd / zoom.
///
/// All fields here are populated from event payloads (`PaneUpdate`,
/// `CwdChanged`, `SessionUpdate`) — never from synchronous shim calls.
/// Synchronous shims (`get_pane_cwd`, `get_pane_running_command`) block
/// the `plugin-exec-N` worker and cascade into `Action NewPane` SLA
/// timeouts, so the plugin MUST stay event-driven.
#[derive(Default)]
struct ActivePaneState {
    /// Last-seen focused terminal pane id. A change triggers a fresh
    /// write of the `.cwd` / `.cmd` / `.zoom` files.
    pane_id: Option<u32>,
    /// Last-written fullscreen state of the focused pane. Drives the
    /// zoom indicator in the status bar's left segment.
    fullscreen: bool,
    /// Last-written title of the focused pane. Drives the `.cmd` display.
    title: Option<String>,
    /// Last-written CWD (already formatted for display). Drives `.cwd`.
    cwd_display: Option<String>,
    /// pane_id → cwd string. Populated by two sources:
    ///   1. `muxline::cwd::<pid>` pipe messages from zsh precmd
    ///      (near-instant, updates on every prompt / `cd`).
    ///   2. `Event::CwdChanged` as a slower fallback for non-zsh panes.
    cwds: HashMap<u32, String>,
    /// pane_id → foreground command name. Populated by
    /// `muxline::cmd::<pid>` pipe messages from zsh preexec/precmd —
    /// tmux-style `pane_current_command` equivalent without requiring a
    /// `get_pane_running_command` shim call (which blocks the plugin worker
    /// and was the root cause of the original pane-open cascade). Falls
    /// back to `PaneInfo.title` when absent.
    cmds: HashMap<u32, String>,
    /// Session name captured from SessionUpdate.
    session_name: Option<String>,
    /// $HOME at load — used to display `~` for the home directory.
    home: Option<String>,
    /// Plugin ids of every zjstatus instance in this session — discovered
    /// from `SessionInfo.plugins` on SessionUpdate. The layout uses
    /// `default_tab_template` to instantiate a zjstatus in every tab, so
    /// there are N zjstatus plugins for N tabs; we broadcast each pipe
    /// update to all of them so every tab's status bar stays in sync with
    /// the active pane.
    ///
    /// Targeting by id is required because `pipe_message_to_plugin`'s
    /// `with_plugin_url` path expects the exact wasm URL and silently
    /// drops when given an alias like `"zjstatus"`. Empty until the first
    /// SessionUpdate carries the plugin manifest.
    zjstatus_plugin_ids: Vec<u32>,
}

#[derive(Default)]
struct State {
    permissions_granted: bool,
    tabs: Vec<TabInfo>,
    panes: PaneManifest,
    notification_state: HashMap<u32, HashSet<NotificationType>>,
    pending_renames: HashSet<usize>,
    config: Config,
    updating_tabs: bool,
    mode: Mode,
    rename: RenameState,
    active: ActivePaneState,
}

impl State {
    fn is_default_name(&self, name: &str) -> bool {
        if let Some(suffix) = name.strip_prefix("Tab #") {
            suffix.chars().all(|c| c.is_ascii_digit()) && !suffix.is_empty()
        } else {
            false
        }
    }

    fn strip_icons(&self, name: &str) -> String {
        let mut result = name.to_string();
        for icon in [&self.config.waiting_icon, &self.config.completed_icon] {
            let suffix = format!(" {}", icon);
            while result.ends_with(&suffix) {
                result.truncate(result.len() - suffix.len());
            }
        }
        result
    }

    fn normalize_base(&self, current_name: &str) -> String {
        let stripped = self.strip_icons(current_name);
        if stripped.is_empty() || self.is_default_name(&stripped) {
            String::new()
        } else if stripped.starts_with(USER_NAME_PREFIX) {
            stripped
        } else {
            format!("{}{}", USER_NAME_PREFIX, stripped)
        }
    }

    fn display_name(&self, current_name: &str) -> String {
        let base = self.normalize_base(current_name);
        base.strip_prefix(USER_NAME_PREFIX)
            .unwrap_or(&base)
            .to_string()
    }

    /// The currently-focused PaneInfo (terminal pane only — plugins excluded).
    /// Reads from the cached `PaneManifest`, which is updated by PaneUpdate
    /// events. No shim calls.
    fn focused_pane_info(&self) -> Option<&PaneInfo> {
        let active_tab = self.tabs.iter().find(|t| t.active)?;
        let panes = self.panes.panes.get(&active_tab.position)?;
        panes.iter().find(|p| {
            !p.is_plugin
                && p.is_focused
                && p.is_floating == active_tab.are_floating_panes_visible
        })
    }

    // ---- Active-pane file publishing (Indicator mode) ----------------
    // Plugin's `/tmp/` maps to zellij's tmp dir on host — see zellij-server's
    // plugin_loader.rs preopened_dir. On macOS that's `$TMPDIR/zellij-$UID/`;
    // on Linux it's `/tmp/zellij-$UID/`. The zjstatus command widgets read
    // from the same host path, so the plugin + zjstatus agree on location
    // without touching the real /tmp.

    /// Basename-trim + `~` substitution + 32-char ellipsis. Produces the
    /// display string written to the `.cwd` file.
    fn format_cwd_display(&self, cwd: &str) -> String {
        let home = self.active.home.as_deref();
        if home.map(|h| h == cwd).unwrap_or(false) {
            return "~".to_string();
        }
        let basename = cwd.rsplit('/').find(|s| !s.is_empty()).unwrap_or(cwd);
        if basename.chars().count() > 32 {
            let tail: String = basename.chars().rev().take(29).collect();
            format!("...{}", tail.chars().rev().collect::<String>())
        } else {
            basename.to_string()
        }
    }

    /// Fallback command display derived from a pane's title, used only
    /// when the zsh-pipe path hasn't produced a `cmds` entry yet (e.g.,
    /// pane just opened, first prompt not yet fired, or pane is running a
    /// non-zsh shell).
    ///
    /// Oh-my-zsh's termsupport sets the pane title to `user@host: <path>`
    /// at a prompt and `user@host: <cmd>` during execution. Naively
    /// tokenizing the title would display the machine prefix (what the
    /// user flagged). Strategy:
    ///   1. Default names ("Pane #N", "Tab #N") or empty → "zsh".
    ///   2. If the title contains `@` (oh-my-zsh format), take everything
    ///      after the rightmost `:` — that's either the cwd or the cmd.
    ///   3. If that tail looks like a path (starts with `/` or `~`) the
    ///      shell is idle → show "zsh".
    ///   4. Otherwise show the first whitespace-separated token of the
    ///      tail — the command name, matching tmux's
    ///      `pane_current_command`.
    fn format_cmd_display(title: &str) -> String {
        let trimmed = title.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Pane #")
            || trimmed.starts_with("Tab #")
        {
            return "zsh".to_string();
        }
        let tail = if trimmed.contains('@') {
            trimmed.rsplit(':').next().unwrap_or(trimmed).trim()
        } else {
            trimmed
        };
        if tail.is_empty() || tail.starts_with('/') || tail.starts_with('~') {
            return "zsh".to_string();
        }
        tail.split_whitespace().next().unwrap_or("zsh").to_string()
    }

    /// Zoom segment matching tmux's `status-left`:
    ///   #{?window_zoomed_flag,│  zoom ,}
    /// Catppuccin Mocha overlay_0 (#6c7086) for the separator,
    /// yellow (#f9e2af) for the Nerd Font expand glyph (U+EB7F) + label.
    /// Empty when not fullscreen.
    fn format_zoom_display(is_fullscreen: bool) -> &'static str {
        if is_fullscreen {
            "#[fg=#6c7086]│#[fg=#f9e2af] \u{eb7f} zoom "
        } else {
            ""
        }
    }

    /// Evaluate the currently-focused pane against the last-published
    /// state and write ONLY the files whose displayed value changed.
    /// Called from every PaneUpdate and every relevant CwdChanged.
    ///
    /// Zero shim calls — all inputs come from event payloads.
    fn publish_focused_pane(&mut self) {
        // `TabInfo.is_fullscreen_active` is the authoritative signal for
        // `Alt+z` (ToggleFocusFullscreen) — it's tab-level, not pane-level,
        // and gets set reliably when the toggle action fires.
        // `PaneInfo.is_fullscreen` is undocumented and does NOT track
        // `Alt+z` in practice.
        let fullscreen = self
            .tabs
            .iter()
            .find(|t| t.active)
            .map(|t| t.is_fullscreen_active)
            .unwrap_or(false);
        let Some(focused) = self.focused_pane_info() else {
            return;
        };
        let pane_id = focused.id;
        let title = focused.title.clone();
        let cwd_display = self
            .active
            .cwds
            .get(&pane_id)
            .map(|c| self.format_cwd_display(c))
            .unwrap_or_else(|| "~".to_string());

        let focus_changed = self.active.pane_id != Some(pane_id);
        self.active.pane_id = Some(pane_id);

        // All three segments go to zjstatus via `pipe_message_to_plugin`
        // (`src/pipe.rs:parse_protocol` updates `pipe_results` synchronously
        // AND returns `should_render=true`, so fresh content lands in the
        // very next render). The file-based `command_*` widget path is
        // unsuitable here: (1) zjstatus v0.22.0's `Event::RunCommandResult`
        // handler does NOT set `should_render=true`
        // (src/bin/zjstatus.rs:229-260) so it races the render, and (2) the
        // spawned `cat` depends on zjstatus's subprocess env carrying
        // `$ZELLIJ_SESSION_NAME`, which isn't guaranteed.
        //
        // `pipe_message_to_plugin` requires identifying the target.
        // `with_plugin_url("zjstatus")` (the alias) silently drops — the
        // routing path expects the exact wasm URL. We target by
        // `destination_plugin_id` for every zjstatus instance discovered
        // on SessionUpdate (one per tab, since zjstatus lives in
        // `default_tab_template`). If none have been observed yet, skip
        // both the pipe AND the state bookkeeping so the next event
        // retries cleanly.
        if self.active.zjstatus_plugin_ids.is_empty() {
            return;
        }

        if focus_changed || self.active.cwd_display.as_deref() != Some(&cwd_display) {
            self.broadcast_to_zjstatus("pipe_cwd", &cwd_display);
            self.active.cwd_display = Some(cwd_display);
        }

        // Prefer the zsh-pipe `cmds` entry (tmux-parity: exact current
        // foreground command, no machine-prefix noise). Fall back to the
        // pane title heuristic when a pane hasn't piped yet.
        let cmd_display = self
            .active
            .cmds
            .get(&pane_id)
            .cloned()
            .unwrap_or_else(|| Self::format_cmd_display(&title));
        if focus_changed || self.active.title.as_ref() != Some(&cmd_display) {
            self.broadcast_to_zjstatus("pipe_cmd", &cmd_display);
            self.active.title = Some(cmd_display);
        }

        if focus_changed || self.active.fullscreen != fullscreen {
            let segment = Self::format_zoom_display(fullscreen);
            self.broadcast_to_zjstatus("pipe_zoom", segment);
            self.active.fullscreen = fullscreen;
        }
    }

    /// Send a `zjstatus::pipe::<name>::<content>` message to every known
    /// zjstatus instance in this session. Each tab has its own zjstatus
    /// plugin (via `default_tab_template`), so broadcasting keeps every
    /// tab's status bar in sync with the active pane.
    fn broadcast_to_zjstatus(&self, pipe_name: &str, content: &str) {
        let payload = format!("zjstatus::pipe::{}::{}", pipe_name, content);
        for &zj_id in &self.active.zjstatus_plugin_ids {
            pipe_message_to_plugin(
                MessageToPlugin::new("muxline-update")
                    .with_destination_plugin_id(zj_id)
                    .with_payload(payload.clone()),
            );
        }
    }

    /// Prune cached CWDs and CMDs for panes that no longer exist (closed).
    fn prune_closed_pane_cwds(&mut self) {
        if (self.active.cwds.is_empty() && self.active.cmds.is_empty())
            || self.panes.panes.is_empty()
        {
            return;
        }
        let alive: HashSet<u32> = self
            .panes
            .panes
            .values()
            .flat_map(|ps| ps.iter().filter(|p| !p.is_plugin).map(|p| p.id))
            .collect();
        self.active.cwds.retain(|id, _| alive.contains(id));
        self.active.cmds.retain(|id, _| alive.contains(id));
    }

    fn clear_focused(&mut self) -> bool {
        if let Some(pane_id) = self.focused_pane_info().map(|p| p.id) {
            self.notification_state.remove(&pane_id).is_some()
        } else {
            false
        }
    }

    fn clean_stale(&mut self) {
        if self.notification_state.is_empty() || self.panes.panes.is_empty() {
            return;
        }
        let current: HashSet<u32> = self
            .panes
            .panes
            .values()
            .flat_map(|ps| ps.iter().filter(|p| !p.is_plugin).map(|p| p.id))
            .collect();
        self.notification_state.retain(|id, _| current.contains(id));
    }

    fn tab_notification(&self, tab_position: usize) -> Option<NotificationType> {
        let panes = self.panes.panes.get(&tab_position)?;
        let mut has_completed = false;
        for pane in panes.iter().filter(|p| !p.is_plugin) {
            if let Some(n) = self.notification_state.get(&pane.id) {
                if n.contains(&NotificationType::Waiting) {
                    return Some(NotificationType::Waiting);
                }
                if n.contains(&NotificationType::Completed) {
                    has_completed = true;
                }
            }
        }
        if has_completed {
            Some(NotificationType::Completed)
        } else {
            None
        }
    }

    fn reconcile_tab_names(&mut self) {
        if self.updating_tabs {
            return;
        }
        self.updating_tabs = true;

        let positions_and_desired: Vec<(usize, String, String)> = self
            .tabs
            .iter()
            .map(|tab| {
                let base = self.normalize_base(&tab.name);
                let icon_suffix = if self.config.enabled {
                    match self.tab_notification(tab.position) {
                        Some(NotificationType::Waiting) => {
                            format!(" {}", self.config.waiting_icon)
                        },
                        Some(NotificationType::Completed) => {
                            format!(" {}", self.config.completed_icon)
                        },
                        None => String::new(),
                    }
                } else {
                    String::new()
                };
                let desired = format!("{}{}", base, icon_suffix);
                (tab.position, tab.name.clone(), desired)
            })
            .collect();

        for (position, current, desired) in positions_and_desired {
            if current != desired {
                self.pending_renames.insert(position);
                rename_tab((position + 1) as u32, &desired);
            } else {
                self.pending_renames.remove(&position);
            }
        }

        let valid: HashSet<usize> = self.tabs.iter().map(|t| t.position).collect();
        self.pending_renames.retain(|p| valid.contains(p));

        self.updating_tabs = false;
    }

    // ---- RenameTab mode ------------------------------------------------

    /// Capture the active tab's id + stripped name for RenameTab mode.
    fn capture_tab_target(&mut self) {
        if self.rename.captured {
            return;
        }
        if let Some(active) = self.tabs.iter().find(|t| t.active) {
            self.rename.target_tab_id = Some(active.tab_id);
            self.rename.editor.set(&self.display_name(&active.name));
            self.rename.captured = true;
        }
    }

    /// Capture the current session's name from a SessionUpdate payload.
    fn capture_session_target(&mut self, sessions: &[SessionInfo]) {
        if self.rename.captured {
            return;
        }
        if let Some(current) = sessions.iter().find(|s| s.is_current_session) {
            self.rename.editor.set(&current.name);
            self.rename.captured = true;
        }
    }

    /// Size + center the floating pane based on the active tab's display area.
    /// Re-runs on every TabUpdate until at least one valid size has been applied.
    fn size_self_from_active(&mut self) {
        if self.rename.sized {
            return;
        }
        let Some(active) = self.tabs.iter().find(|t| t.active) else {
            return;
        };
        let tab_cols = active.display_area_columns;
        let tab_rows = active.display_area_rows;
        if tab_cols == 0 || tab_rows == 0 {
            return;
        }
        let plugin_id = get_plugin_ids().plugin_id;
        let width = MODAL_COLS.min(tab_cols.saturating_sub(4)).max(20);
        let height = MODAL_ROWS;
        let x = tab_cols.saturating_sub(width) / 2;
        let y = tab_rows.saturating_sub(height) / 2;
        if let Some(coords) = FloatingPaneCoordinates::new(
            Some(x.to_string()),
            Some(y.to_string()),
            Some(width.to_string()),
            Some(height.to_string()),
            None,
            None,
        ) {
            change_floating_panes_coordinates(vec![(PaneId::Plugin(plugin_id), coords)]);
            self.rename.sized = true;
        }
    }

    fn submit_rename(&self) {
        let text = self.rename.editor.text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        match self.mode {
            Mode::RenameTab => {
                if let Some(tab_id) = self.rename.target_tab_id {
                    rename_tab_with_id(tab_id as u64, trimmed);
                }
            },
            Mode::RenameSession => {
                rename_session(trimmed);
            },
            _ => {},
        }
    }

    fn handle_key(&mut self, k: KeyWithModifier) {
        let ctrl = k.key_modifiers.contains(&KeyModifier::Ctrl);
        let alt = k.key_modifiers.contains(&KeyModifier::Alt);
        let ed = &mut self.rename.editor;

        match k.bare_key {
            // ---- submit / cancel ----
            BareKey::Enter => {
                self.submit_rename();
                close_self();
                return;
            },
            BareKey::Esc => {
                close_self();
                return;
            },

            // ---- movement ----
            BareKey::Left if alt || ctrl => ed.move_word_left(),
            BareKey::Right if alt || ctrl => ed.move_word_right(),
            BareKey::Left => ed.move_left(),
            BareKey::Right => ed.move_right(),
            BareKey::Home => ed.home(),
            BareKey::End => ed.end(),
            BareKey::Char('a') if ctrl => ed.home(),
            BareKey::Char('e') if ctrl => ed.end(),
            BareKey::Char('b') if ctrl => ed.move_left(),
            BareKey::Char('f') if ctrl => ed.move_right(),
            BareKey::Char('b') if alt => ed.move_word_left(),
            BareKey::Char('f') if alt => ed.move_word_right(),

            // ---- editing ----
            BareKey::Backspace if alt => ed.kill_word_backward(),
            BareKey::Backspace => ed.backspace(),
            BareKey::Delete => ed.delete(),
            BareKey::Char('d') if ctrl => ed.delete(),
            BareKey::Char('d') if alt => ed.kill_word_forward(),
            BareKey::Char('w') if ctrl => ed.kill_word_backward(),
            BareKey::Char('u') if ctrl => ed.kill_to_start(),
            BareKey::Char('k') if ctrl => ed.kill_to_end(),

            // ---- insertion ----
            BareKey::Char(c) if !ctrl && !alt => ed.insert(c),

            _ => {},
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.mode = Mode::from_configuration(&configuration);
        self.config = Config::from_configuration(&configuration);
        // $HOME is used to render the CWD as `~` when the user is in their
        // home dir. Captured once at load — it doesn't change.
        self.active.home = std::env::var("HOME").ok();
        let mut perms = vec![
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ];
        if self.mode == Mode::Indicator {
            // Indicator mode feeds the status bar by pushing the cwd/cmd/zoom
            // segments directly to zjstatus via `pipe_message_to_plugin`.
            // Without this permission the shim returns silently but zellij
            // logs `permission 'MessageAndLaunchOtherPlugins' denied` and
            // no pipe message ever reaches zjstatus.
            perms.push(PermissionType::MessageAndLaunchOtherPlugins);
        }
        if self.mode == Mode::ReloadConfig {
            // `run_command` shells out to `cat ~/.config/zellij/config.kdl`.
            perms.push(PermissionType::RunCommands);
        }
        request_permission(&perms);
        let events = match self.mode {
            Mode::Indicator => vec![
                EventType::PermissionRequestResult,
                EventType::TabUpdate,
                EventType::PaneUpdate,
                // Needed so the status bar can reflect the FOCUSED pane's
                // cwd/cmd (not just whichever one last ran a shell prompt).
                EventType::SessionUpdate,
                EventType::CwdChanged,
            ],
            Mode::RenameTab => vec![
                EventType::PermissionRequestResult,
                EventType::TabUpdate,
                EventType::Key,
            ],
            Mode::RenameSession => vec![
                EventType::PermissionRequestResult,
                EventType::TabUpdate,
                EventType::SessionUpdate,
                EventType::Key,
            ],
            Mode::ReloadConfig => vec![
                EventType::PermissionRequestResult,
                EventType::RunCommandResult,
            ],
        };
        subscribe(&events);
        eprintln!(
            "muxline: v{} loaded (mode={:?})",
            env!("CARGO_PKG_VERSION"),
            self.mode
        );
    }

    fn update(&mut self, event: Event) -> bool {
        match (self.mode, event) {
            (_, Event::PermissionRequestResult(status)) => {
                self.permissions_granted = status == PermissionStatus::Granted;
                match self.mode {
                    Mode::Indicator => {
                        set_selectable(false);
                        self.reconcile_tab_names();
                        true
                    },
                    Mode::RenameTab => {
                        set_selectable(true);
                        let plugin_id = get_plugin_ids().plugin_id;
                        rename_plugin_pane(plugin_id, "Rename Tab");
                        true
                    },
                    Mode::RenameSession => {
                        set_selectable(true);
                        let plugin_id = get_plugin_ids().plugin_id;
                        rename_plugin_pane(plugin_id, "Rename Session");
                        true
                    },
                    Mode::ReloadConfig => {
                        // Don't steal focus — plugin is invisible and closes
                        // itself as soon as RunCommandResult comes back.
                        set_selectable(false);
                        // Fire-and-forget: `sh -c` so `~` expands to $HOME.
                        // Result arrives asynchronously as RunCommandResult.
                        run_command(
                            &["sh", "-c", "cat ~/.config/zellij/config.kdl"],
                            BTreeMap::new(),
                        );
                        false
                    },
                }
            },
            (Mode::ReloadConfig, Event::RunCommandResult(exit, stdout, _stderr, _ctx)) => {
                if exit == Some(0) {
                    if let Ok(contents) = String::from_utf8(stdout) {
                        // write_to_disk=false: user edits the dotfile directly,
                        // we just re-read it. Mirrors `source-file` semantics.
                        reconfigure(contents, false);
                    }
                } else {
                    eprintln!(
                        "muxline: reload_config cat failed (exit={:?})",
                        exit
                    );
                }
                close_self();
                false
            },
            (Mode::Indicator, Event::TabUpdate(tabs)) => {
                self.tabs = tabs;
                self.clear_focused();
                self.clean_stale();
                self.reconcile_tab_names();
                // Alt+z (ToggleFocusFullscreen) flips TabInfo.is_fullscreen_active.
                // That change can arrive via TabUpdate without a concurrent
                // PaneUpdate, so publish here too — `publish_focused_pane`
                // is change-detected internally and short-circuits if nothing
                // moved.
                self.publish_focused_pane();
                false
            },
            (Mode::Indicator, Event::PaneUpdate(panes)) => {
                self.panes = panes;
                self.clear_focused();
                self.clean_stale();
                self.prune_closed_pane_cwds();
                self.reconcile_tab_names();
                // All display inputs (focus, title, is_fullscreen) come from
                // the PaneManifest we just cached. Zero shim calls.
                self.publish_focused_pane();
                false
            },
            (Mode::Indicator, Event::SessionUpdate(sessions, _)) => {
                if let Some(current) = sessions.iter().find(|s| s.is_current_session) {
                    let mut state_changed = false;

                    let new_name = Some(current.name.clone());
                    if new_name != self.active.session_name {
                        self.active.session_name = new_name;
                        state_changed = true;
                    }

                    // Discover every zjstatus instance's plugin id by
                    // matching its location substring. One zjstatus is
                    // instantiated per tab (via `default_tab_template`), so
                    // this set grows as tabs are created. Substring match
                    // tolerates whichever spelling the location carries —
                    // "zjstatus" alias, full GitHub release URL, etc.
                    let mut new_zj_ids: Vec<u32> = current
                        .plugins
                        .iter()
                        .filter(|(_, info)| info.location.contains("zjstatus"))
                        .map(|(id, _)| *id)
                        .collect();
                    new_zj_ids.sort_unstable();
                    if new_zj_ids != self.active.zjstatus_plugin_ids {
                        self.active.zjstatus_plugin_ids = new_zj_ids;
                        state_changed = true;
                    }

                    if state_changed {
                        // Force a republish: clear last-sent markers so
                        // publish_focused_pane re-pipes cwd/cmd/zoom with
                        // the freshly-known destination.
                        self.active.cwd_display = None;
                        self.active.title = None;
                        self.publish_focused_pane();
                    }
                }
                false
            },
            (Mode::Indicator, Event::CwdChanged(pane_id, new_cwd, _clients)) => {
                // Cache the new cwd for this pane (event payload — no shim).
                // Only re-publish the .cwd file if the pane is focused;
                // background panes update silently in the cache.
                if let PaneId::Terminal(id) = pane_id {
                    self.active
                        .cwds
                        .insert(id, new_cwd.to_string_lossy().into_owned());
                    if Some(id) == self.active.pane_id {
                        self.publish_focused_pane();
                    }
                }
                false
            },
            (Mode::RenameTab, Event::TabUpdate(tabs)) => {
                self.tabs = tabs;
                self.capture_tab_target();
                self.size_self_from_active();
                true
            },
            (Mode::RenameSession, Event::TabUpdate(tabs)) => {
                self.tabs = tabs;
                self.size_self_from_active();
                true
            },
            (Mode::RenameSession, Event::SessionUpdate(sessions, _resurrectable)) => {
                self.capture_session_target(&sessions);
                true
            },
            (mode, Event::Key(k)) if mode.is_rename() => {
                self.handle_key(k);
                true
            },
            _ => false,
        }
    }

    fn render(&mut self, _rows: usize, cols: usize) {
        match self.mode {
            // Invisible background modes — nothing to draw.
            Mode::Indicator | Mode::ReloadConfig => {},
            Mode::RenameTab | Mode::RenameSession => {
                // Floating plugin panes have a 1-cell frame on every side, so
                // content is 1 row tall. The frame title ("Rename Tab" or
                // "Rename Session") is set via `rename_plugin_pane` on load,
                // so no in-pane label is needed — we render the input alone.
                //
                // Raw ANSI end-to-end (no `print_text_with_coordinates`, no
                // `show_cursor` shim):
                //   - One `print!` per render, one `flush`, one host read.
                //   - Cursor rendered inline as an inverted-video character,
                //     so it's always visible regardless of whether zellij
                //     paints a terminal cursor for plugin panes.
                //   - `\u{1b}[2K` erases the content row each frame, so buffer
                //     shrinks (backspace / kill-word) don't leave stale chars
                //     and nothing accumulates into scrollback.
                let buf: Vec<char> = self.rename.editor.buffer.clone();
                let cursor_pos = self.rename.editor.cursor;

                let (scroll, cursor_in_view) =
                    scroll_for_cursor(cursor_pos, buf.len(), cols.max(1));

                // SGR 0 = reset, 7 = reverse video, 27 = no reverse. Reverse
                // video inherits the pane's default fg/bg, so it works in any
                // theme.
                let mut out = String::with_capacity(cols + 32);
                // Go to row 1, col 1; reset SGR; clear whole line.
                out.push_str("\u{1b}[1;1H\u{1b}[0m\u{1b}[2K");

                // Render the visible slice, inverting the cell at the cursor.
                let end = (scroll + cols).min(buf.len());
                for (i, c) in buf[scroll..end].iter().enumerate() {
                    if i == cursor_in_view {
                        out.push_str("\u{1b}[7m");
                        out.push(*c);
                        out.push_str("\u{1b}[27m");
                    } else {
                        out.push(*c);
                    }
                }
                // Cursor past end of buffer → render it as an inverted space.
                if cursor_in_view >= end.saturating_sub(scroll) {
                    out.push_str("\u{1b}[7m \u{1b}[27m");
                }

                print!("{}", out);
                // CSI sequences don't end in '\n' so LineWriter never
                // auto-flushes — explicit flush pushes bytes into the WASI
                // stdout pipe before zellij's wasi_read_string runs.
                let _ = std::io::stdout().flush();
            },
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if self.mode != Mode::Indicator {
            return false;
        }
        let message = if pipe_message.name.starts_with("muxline::") {
            pipe_message.name.clone()
        } else if let Some(ref payload) = pipe_message.payload {
            if payload.starts_with("muxline::") {
                payload.clone()
            } else {
                return false;
            }
        } else {
            return false;
        };

        let parts: Vec<&str> = message.split("::").collect();
        if parts.len() < 3 {
            eprintln!(
                "muxline: Invalid message format (expect muxline::EVENT::PANE_ID): {}",
                message
            );
            unblock_cli_pipe_input(&pipe_message.name);
            return false;
        }

        let event_type = parts[1];
        let pane_id: u32 = match parts[2].parse() {
            Ok(n) => n,
            Err(_) => {
                eprintln!("muxline: Invalid pane_id '{}'", parts[2]);
                unblock_cli_pipe_input(&pipe_message.name);
                return false;
            },
        };

        // CLI-pipe payload: present for `cwd` / `cmd` events which carry
        // the $PWD or command name. `waiting` / `completed` don't use it.
        let payload = pipe_message.payload.clone();
        unblock_cli_pipe_input(&pipe_message.name);

        match event_type.to_lowercase().as_str() {
            "waiting" | "completed" => {
                let notification = if event_type == "waiting" {
                    NotificationType::Waiting
                } else {
                    NotificationType::Completed
                };
                let mut ns = HashSet::new();
                ns.insert(notification);
                self.notification_state.insert(pane_id, ns);
                self.reconcile_tab_names();
            },
            // Zsh precmd → `zellij pipe --name muxline::cwd::<pid> --payload $PWD`.
            // Fires at every prompt so `cd` reflects in the status bar in
            // one prompt-tick (no dependency on zellij's CwdChanged polling
            // which has a ~1s interval).
            "cwd" => {
                if let Some(cwd) = payload {
                    self.active.cwds.insert(pane_id, cwd);
                    if Some(pane_id) == self.active.pane_id {
                        self.publish_focused_pane();
                    }
                }
            },
            // Zsh preexec → `zellij pipe --name muxline::cmd::<pid> --payload <cmd>`
            // with the first token of the command being run. Zsh precmd
            // emits `zsh` again to reset at the next prompt. Matches
            // tmux's `pane_current_command` behavior.
            "cmd" => {
                if let Some(cmd) = payload {
                    let trimmed = cmd.trim();
                    if trimmed.is_empty() {
                        self.active.cmds.remove(&pane_id);
                    } else {
                        self.active.cmds.insert(pane_id, trimmed.to_string());
                    }
                    if Some(pane_id) == self.active.pane_id {
                        self.publish_focused_pane();
                    }
                }
            },
            other => {
                eprintln!("muxline: Unknown event type '{}'", other);
            },
        }
        false
    }
}

/// Given cursor pos, total length, and visible width, return (scroll_offset,
/// cursor_column_within_visible).
fn scroll_for_cursor(cursor: usize, len: usize, width: usize) -> (usize, usize) {
    if len <= width {
        return (0, cursor);
    }
    if cursor < width {
        return (0, cursor);
    }
    // Keep cursor one column from the right edge when scrolling.
    let right_pad = 1;
    let scroll = cursor + right_pad + 1 - width;
    let scroll = scroll.min(len + right_pad - width);
    (scroll, cursor - scroll)
}

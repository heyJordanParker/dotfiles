"""Shared library for the Claude Code / Codex automation hooks.

The hooks are flat entry-point scripts in the parent directory, invoked by
absolute path. The reusable spine they compose from lives here: payload and
command parsing, the transcript layer, the session-state store, and the model
call. Hooks import it as `from lib import x`
or `from lib.x import y`; that resolves because the hook's own directory is on
sys.path[0] when it is run by path, and `lib` is a package beneath it.
"""

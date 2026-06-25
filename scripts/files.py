"""Write a generated file only when its content changed.

The sync generators (agents.py, hooks.py) rebuild their output on every run.
Overwriting with identical bytes bumps the mtime and, for the stowed codex
config.toml, makes the app re-read it for nothing. write_if_changed compares the
new text against what is on disk and writes only on a difference.
"""


def write_if_changed(path, text):
    try:
        with open(path, encoding="utf-8") as f:
            if f.read() == text:
                return False
    except FileNotFoundError:
        pass
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    return True

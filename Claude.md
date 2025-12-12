# macOS Terminal Keybindings Reference

Avoid these when adding custom keybindings - they're system defaults.

## Ctrl (readline/shell)
- ^a - beginning of line
- ^e - end of line
- ^b - back one char
- ^f - forward one char
- ^d - delete forward
- ^h - delete backward
- ^k - kill to end of line
- ^u - kill whole line
- ^w - kill word backward
- ^y - yank (paste killed text)
- ^t - transpose chars
- ^p - previous history
- ^n - next history
- ^r - reverse search history
- ^l - clear screen
- ^c - interrupt
- ^z - suspend
- ^i - tab (same keycode)
- ^j - newline (same keycode)
- ^m - return (same keycode)

## Alt/Option (word movement)
- ~b - back one word
- ~f - forward one word
- ~d - delete word forward
- ~Delete - delete word backward
- ~Enter - insert newline
- ~Tab - insert tab
- ~Esc - complete

## Cmd - typically handled by terminal app, not shell
- Cmd+c - copy
- Cmd+v - paste
- Cmd+a - select all
- Cmd+. - cancel

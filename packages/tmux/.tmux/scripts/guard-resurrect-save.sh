#!/bin/bash
# Prevents resurrect from overwriting a good save with a near-empty one.
# Used as @resurrect-hook-post-save-layout — fires after the file is written
# but before the `last` symlink is updated.

new_file="$1"
resurrect_dir="$(dirname "$new_file")"
last="$resurrect_dir/last"
old_file="$(readlink "$last" 2>/dev/null)"

[ -z "$old_file" ] && exit 0

old_path="$resurrect_dir/$old_file"
[ -f "$old_path" ] || exit 0

old_size=$(wc -c < "$old_path" 2>/dev/null)
new_size=$(wc -c < "$new_file" 2>/dev/null)

# If old save was substantial (>1KB) and new save dropped below 20% of it,
# overwrite the new file with the old content so `files_differ` sees no change
# and the symlink stays put.
if [ "$old_size" -gt 1000 ] && [ "$new_size" -lt $((old_size / 5)) ]; then
  cp "$old_path" "$new_file"
fi

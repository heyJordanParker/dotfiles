"""tracer CLI entry point.

Registers each command from the `commands` package on a single click group.
Every command except `doctor` calls `require_dependencies()` before doing
work — if any required external binary is missing, the command exits 1
with per-platform install instructions.
"""

from __future__ import annotations

import click

from tracer.commands import (
    blame,
    cache,
    callers,
    context,
    defines,
    diff,
    doctor,
    downstream,
    find,
    grep,
    history,
    info,
    list_,
    read,
    status,
    struct_,
    structure,
    survey,
    symbols,
    tree,
    upstream,
)


@click.group()
@click.version_option()
def main() -> None:
    """Code intelligence CLI for mapping architectural relationships."""


main.add_command(doctor.command, name="doctor")
main.add_command(read.command, name="read")
main.add_command(tree.command, name="tree")
main.add_command(survey.command, name="survey")
main.add_command(info.command, name="info")
main.add_command(structure.command, name="structure")
main.add_command(grep.command, name="grep")
main.add_command(struct_.command, name="struct")
main.add_command(callers.command, name="callers")
main.add_command(defines.command, name="defines")
main.add_command(symbols.command, name="symbols")
main.add_command(upstream.command, name="upstream")
main.add_command(downstream.command, name="downstream")
main.add_command(history.command, name="history")
main.add_command(blame.command, name="blame")
main.add_command(cache.command, name="cache")
main.add_command(list_.command, name="list")
main.add_command(context.command, name="context")
main.add_command(status.command, name="status")
main.add_command(find.command, name="find")
main.add_command(diff.command, name="diff")


if __name__ == "__main__":
    main()

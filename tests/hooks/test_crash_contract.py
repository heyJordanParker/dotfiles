import contextlib
import importlib.util
import inspect
import io
import os
import signal
import subprocess
import sys
from pathlib import Path

import pytest

HOOKS = Path(__file__).parents[2] / "packages" / "agents" / "hooks"
PAYLOADS = ("", "not json", "{}", '{"value":"' + "x" * (1024 * 1024) + '"}')
TIMEOUT_SECONDS = 1


def hook_scripts():
    return sorted(HOOKS.glob("*.py"))


def run_hook_in_process(script, payload):
    module_name = f"crash_contract_{script.stem}"
    spec = importlib.util.spec_from_file_location(module_name, script)
    module = importlib.util.module_from_spec(spec)
    stdin = io.StringIO(payload)
    stdout = io.StringIO()
    stderr = io.StringIO()

    def timeout_handler(signum, frame):
        raise TimeoutError(f"exceeded {TIMEOUT_SECONDS}s")

    previous_handler = signal.signal(signal.SIGALRM, timeout_handler)
    previous_environment = os.environ.copy()
    previous_directory = os.getcwd()
    signal.setitimer(signal.ITIMER_REAL, TIMEOUT_SECONDS)
    try:
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            previous_stdin = sys.stdin
            sys.stdin = stdin
            try:
                spec.loader.exec_module(module)
                if not callable(getattr(module, "main", None)):
                    return None
                parameters = inspect.signature(module.main).parameters
                arguments = ([],) if len(parameters) == 1 else ()
                try:
                    exit_code = module.main(*arguments)
                except SystemExit as error:
                    exit_code = error.code
            finally:
                sys.stdin = previous_stdin
    finally:
        os.chdir(previous_directory)
        os.environ.clear()
        os.environ.update(previous_environment)
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous_handler)
        sys.modules.pop(module_name, None)
    return (0 if exit_code is None else exit_code), stderr.getvalue()


def run_hook(script, payload):
    result = run_hook_in_process(script, payload)
    if result is not None:
        return result
    completed = subprocess.run(
        [sys.executable, str(script)],
        input=payload,
        capture_output=True,
        text=True,
        timeout=TIMEOUT_SECONDS,
    )
    return completed.returncode, completed.stderr


@pytest.mark.parametrize("payload", PAYLOADS, ids=("empty", "garbage", "object", "large"))
@pytest.mark.parametrize("script", hook_scripts(), ids=lambda path: path.stem)
def test_hook_does_not_crash_or_hang(script, payload):
    exit_code, stderr = run_hook(script, payload)

    assert exit_code in (0, 2)
    assert "Traceback (most recent call last)" not in stderr

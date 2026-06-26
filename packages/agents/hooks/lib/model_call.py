"""model_call — one shared place that owns which model answers an LLM call.

The classifier and the three validators each used to shell out to the same
`claude -p` call and parse the result, copy-pasted four times. They now all go
through `run_model(effort, *, system_prompt, user_prompt, schema)`, which resolves
the backend, calls it, and returns the parsed JSON dict (or None on any failure).

Backends (default `openai`, set via MODEL_CALL_BACKEND):
- openai — direct ChatGPT Responses API, several times faster. Reads codex's
  stored token from ~/.codex/auth.json. The default; the four gates fire
  constantly, so the faster backend is felt directly.
- claude — `claude -p`, schemaless. The CLI's `--json-schema` mode is broken, so
  the working path asks for the JSON shape in the prompt and parses it out of the
  response. Pin a single run back to it with MODEL_CALL_BACKEND=claude for
  debugging.
- local — the review-prompt CLI. Cannot serve general classification or a bare
  run, so its adapter returns the explicit unsupported error.

Two optional capabilities, both off by default:
- measure — when on, records ONE normalized event per call into a per-session log
  (model_calls.jsonl beside reads.jsonl/skills.jsonl), following the same
  per-session event-log pattern the session-state helper uses. Only fields
  reliable and identical in meaning for every backend are recorded.
- raw — "run this model with all its extra features off." Each adapter maps that
  single intent onto its own scaffolding (claude drops the system prompt).

Stdlib only, matching the other hooks.
"""

import json
import os
import re
import subprocess
import time
import urllib.error
import urllib.request

# --- backend resolution ---------------------------------------------------------

DEFAULT_BACKEND = "openai"

# One unified effort vocabulary, translated to each provider's own accepted value.
# OpenAI reasoning.effort accepts none/minimal/low/medium/high/xhigh; Anthropic
# (claude --effort) accepts low/medium/high/xhigh/max with no "none", so our
# "none" floor clamps up to claude's "low".
EFFORT_LEVELS = ("none", "low", "medium", "high", "max")
_OPENAI_EFFORT = {"none": "none", "low": "low", "medium": "medium", "high": "high", "max": "xhigh"}
_CLAUDE_EFFORT = {"none": "low", "low": "low", "medium": "medium", "high": "high", "max": "max"}


def _resolve_backend(backend):
    """The explicit argument wins; else the env default; else openai."""
    return backend or os.environ.get("MODEL_CALL_BACKEND") or DEFAULT_BACKEND


# --- the JSON shape, asked for in the prompt (claude's schemaless path) ----------

def _shape_line(schema):
    """The one instruction that makes a schemaless backend emit parseable JSON.

    The broken `--json-schema` flag used to enforce this; without it we ask in the
    prompt for the exact shape and forbid fences/prose so the regex extraction is
    clean. `schema` is the same dict/JSON the callers already pass.
    """
    shape = schema if isinstance(schema, str) else json.dumps(schema, separators=(",", ":"))
    return ("Output ONLY one line of minified JSON matching this schema, with no "
            "code fence, no prose, and no newlines inside the JSON:\n" + shape)


def _extract_json(text):
    """Pull the JSON object out of a model's text response, or None."""
    if not text:
        return None
    m = re.search(r"\{.*\}", text, re.S)
    if not m:
        return None
    try:
        return json.loads(m.group(0))
    except Exception:
        return None


# --- adapters: each owns its transport, raw mapping, and token normalization -----
# Every adapter returns (parsed_or_None, model_name, raw_flag, input_tokens, output_tokens).
# input_tokens/output_tokens are the normalized comparable counts, or None when
# the backend gave us no usage block.

CLAUDE_MODEL = "opus"


def _call_claude(effort, system_prompt, user_prompt, schema, raw):
    env = dict(os.environ)
    env["CLAUDE_SESSION_HOOK"] = "true"
    cmd = ["claude", "-p", "--model", CLAUDE_MODEL, "--effort", _CLAUDE_EFFORT[effort],
           "--output-format", "json", "--setting-sources", "", "--disallowedTools", "*"]
    # raw = all extra features off: drop the system prompt entirely.
    if not raw:
        cmd += ["--system-prompt", system_prompt]
    prompt = user_prompt + "\n\n" + _shape_line(schema)
    try:
        r = subprocess.run(cmd, input=prompt, capture_output=True, text=True,
                           timeout=120, env=env)
    except Exception:
        return None, CLAUDE_MODEL, raw, None, None
    if r.returncode != 0:
        return None, CLAUDE_MODEL, raw, None, None
    try:
        envelope = json.loads(r.stdout)
    except Exception:
        return None, CLAUDE_MODEL, raw, None, None
    parsed = _extract_json(envelope.get("result", ""))
    inp, out = _claude_tokens(envelope.get("usage"))
    return parsed, CLAUDE_MODEL, raw, inp, out


def _claude_tokens(usage):
    """Total prompt tokens claude processed = fresh + cache-read + cache-creation.

    claude reports the cached portions in separate fields, so they are summed to
    mean the same thing openai's single input_tokens already means.
    """
    if not isinstance(usage, dict):
        return None, None
    inp = (usage.get("input_tokens", 0)
           + usage.get("cache_read_input_tokens", 0)
           + usage.get("cache_creation_input_tokens", 0))
    return inp, usage.get("output_tokens")


OPENAI_MODEL = "gpt-5.5"
_OPENAI_ORIGINATOR = "openclaw"
_OPENAI_URL = "https://chatgpt.com/backend-api/codex/responses"


def _call_openai(effort, system_prompt, user_prompt, schema, raw):
    try:
        auth = json.load(open(os.path.expanduser("~/.codex/auth.json")))
        tok = auth["tokens"]["access_token"]
        acct = auth["tokens"]["account_id"]
    except Exception:
        return None, OPENAI_MODEL, raw, None, None
    prompt = user_prompt + "\n\n" + _shape_line(schema)
    body = {"model": OPENAI_MODEL, "store": False, "stream": True,
            "input": [{"type": "message", "role": "user",
                       "content": [{"type": "input_text", "text": prompt}]}],
            "reasoning": {"effort": _OPENAI_EFFORT[effort], "summary": "auto"},
            "include": ["reasoning.encrypted_content"]}
    # raw = all extra features off: drop the system instructions.
    if not raw:
        body["instructions"] = system_prompt
    req = urllib.request.Request(_OPENAI_URL, data=json.dumps(body).encode(),
        headers={"Authorization": "Bearer %s" % tok, "chatgpt-account-id": acct,
                 "originator": _OPENAI_ORIGINATOR, "OpenAI-Beta": "responses=experimental",
                 "accept": "text/event-stream", "content-type": "application/json",
                 "session_id": "00000000-0000-0000-0000-0000000000ev"}, method="POST")
    final = ""
    usage = None
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            for raw_line in r:
                line = raw_line.decode("utf-8", "replace").strip()
                if not line.startswith("data:"):
                    continue
                p = line[5:].strip()
                if p in ("", "[DONE]"):
                    continue
                ev = json.loads(p)
                if ev.get("type") == "response.output_text.delta":
                    final += ev.get("delta", "")
                u = ev.get("response", {}).get("usage")
                if u:
                    usage = u
    except (urllib.error.HTTPError, Exception):
        return None, OPENAI_MODEL, raw, None, None
    inp, out = _openai_tokens(usage)
    return _extract_json(final), OPENAI_MODEL, raw, inp, out


def _openai_tokens(usage):
    """openai's input_tokens is already the grand total of prompt tokens processed.

    cached_tokens is a subset already counted inside input_tokens, so it is NOT
    added — that would double-count. The single number is the comparable total.
    """
    if not isinstance(usage, dict):
        return None, None
    return usage.get("input_tokens"), usage.get("output_tokens")


_LOCAL_UNSUPPORTED = ("local backend (review-prompt) cannot serve general "
                      "classification or a bare run: its CLI requires review "
                      "instructions")


def _call_local(effort, system_prompt, user_prompt, schema, raw):
    """The review-prompt CLI refuses to run without review instructions, so it
    cannot answer a classification or a bare run. Explicit, not silent."""
    raise NotImplementedError(_LOCAL_UNSUPPORTED)


_ADAPTERS = {"claude": _call_claude, "openai": _call_openai, "local": _call_local}


# --- the one entrypoint the four hooks call -------------------------------------

def run_model(effort="none", *, system_prompt, user_prompt, schema, backend=None,
              measure=False, raw=False, session_id="", hook=""):
    """Resolve the backend, run one model call, return the parsed JSON dict or None.

    effort is one of EFFORT_LEVELS (default "none"), translated to the backend's own
    accepted value. system_prompt / user_prompt / schema are keyword-only and
    required. The backend defaults to openai. With measure on, one normalized event
    is recorded to the per-session log; with measure off, nothing is written. raw
    runs the backend with its extra features off (claude drops the system prompt).
    """
    if effort not in EFFORT_LEVELS:
        raise ValueError("unknown effort %r; valid: %s" % (effort, ", ".join(EFFORT_LEVELS)))
    name = _resolve_backend(backend)
    adapter = _ADAPTERS[name]
    started = time.monotonic()
    parsed, model, raw_flag, inp, out = adapter(effort, system_prompt, user_prompt, schema, raw)
    latency = round(time.monotonic() - started, 3)
    if measure:
        _record(session_id, hook, name, model, raw_flag, latency, inp, out,
                "ok" if parsed else "parse_fail")
    return parsed


# --- optional measurement: one normalized record per call -----------------------

def _record(session_id, hook, backend, model, raw, latency, input_tokens,
            output_tokens, outcome):
    """Append one normalized event to the per-session model_calls.jsonl.

    Follows session_state's per-session event-log pattern: resolve the session
    dir, append one minified JSON line. Only fields reliable and identical in
    meaning for every backend — no cost, no cache split, nothing null for some
    backend.
    """
    import session_state
    if not session_id or not session_state._is_valid_session_id(session_id):
        return
    session_dir = session_state._ensure_session(session_id)
    if session_dir is None:
        return
    entry = session_state._dump({
        "ts": session_state._iso_now(),
        "hook": hook,
        "backend": backend,
        "model": model,
        "raw": raw,
        "latency": latency,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "outcome": outcome,
    })
    with open(os.path.join(session_dir, "model_calls.jsonl"), "a", encoding="utf-8") as fh:
        fh.write(entry + "\n")

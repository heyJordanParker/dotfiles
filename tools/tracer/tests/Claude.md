# WHY

Black-box behavior contract for the `trace` binary, pinning the observable surface that Agents and Skills consume instead of linking against tracer internals.

# Facts

- The Rust test package is named `tracer-cli-tests`.
- The suite drives the `trace` binary as a subprocess.
- `TRACE_BIN` selects the binary under test.
- `TRACE_BIN` defaults to `trace` on `PATH`.
- The suite asserts exit codes, stdout, stderr, JavaScript Object Notation shape, and wall-clock latency.
- `src/lib.rs` is the shared test harness.
- Each test fixture owns a hermetic on-disk git repository.
- `tests/worktree_anchoring.rs` pins the worktree-anchored `.tracer-cache/` contract, and `-C`: it answers for the repository it names, keeps the session record home, carries into every follow-up command, and exits 2 for a missing directory.
- `tests/cache_and_backend.rs` pins the cache lifecycle and the single AST complexity backend.
- `tests/freshness.rs` pins that every call reflects the bytes on disk when it runs.
- `tests/concurrency.rs` pins that concurrent calls serialize index maintenance onto one update and all answer cleanly, and that an index update never waits on another schema's lock.
- `tests/architecture_commands.rs` and `tests/declarations_and_references.rs` pin the relations commands and the resolution model.
- `tests/docs_load.rs` and `tests/docs_graph.rs` pin doc-graph recognition.
- `tests/docs_prime.rs` pins the context-primer auto-load set.
- `tests/session_log.rs` and `tests/docs_load.rs` pin content-hash dedupe in the session log.
- `tests/docs_load.rs` pins user-global Rule scope splitting.
- `tests/document.rs` pins the one `{query, context, results, counts}` shape and the `--filter` contract.
- `tests/enrichment.rs` pins the per-file enrichment every command carries.
- `tests/search.rs`, `tests/git.rs`, and `tests/inspect.rs` pin the search, git, and inspection commands.
- `tests/declarations_and_references.rs` pins PSR-4, `paths`, and Rust crate resolution and the external rule.
- `tests/declarations_and_references.rs` pins PHP lineage: an inherited `$this->save()`, a returned class, and a typed receiver resolving to the nearest trait or parent that declares the method, a call on a base type listing each override `AMBIGUOUS`, and `$this` in a Pest closure read as the test case `tests/Pest.php` binds.
- `tests/declarations_and_references.rs` and `tests/enrichment.rs` pin declaration classification and annotations.
- `tests/architecture_commands.rs` and `tests/primer.rs` pin the `stats` directory table and the primer's sections.
- `tests/enrichment.rs` pins the YAML front matter once per Agent context across `read`, `info`, and `structure`, with its headline and the command's own answer on a repeat, only the lines an edit or a commit changed with a removed fact as `null`, whole again after a reset or for another Agent, every time without a session, whole in `--json`, and whole again for a `structure` file the budget cut to its path; its `directory` block once per Agent, with only the lines a new importer changed on a repeat, a moved `at_session_start` whole, and its entries without what git ignores; a several-file `read` printing every file but the one it cannot answer and recording none it did not print; a `context`, `read`, `info`, or `structure` whose stdout closes before it prints recording nothing as shown; and `--budget` fitting on `read`, `context`, and `docs`.
- `tests/session_log.rs` pins `shown.json` holding each directory block, its entries as their content hash, until a context reset, and a resumed Subagent's first write keeping its archived record.
- `tests/docs_reset.rs` pins that a reset clears shown facts and listings with no `view.json` on disk.
- `tests/declarations_and_references.rs` holds the per-language inventory fixture: one source per language whose declarations cover that language's row set, asserting `header`, `header_line`, `line`, `end_line`, `container`, and `parent` for each.
- `tests/declarations_and_references.rs` pins the ctags fallback on a shell fixture, the data-format deny list, retained Markdown headings, and that a warm `context` or `read` spawns no ctags.
- `tests/enrichment.rs` pins the surface on the first and second `context` of one session, on `read`, `info`, a window, `--offset`/`--limit`, the batch `--json` `results[].content`, `read --at`, and a budget-trimmed read.
- `tests/enrichment.rs` pins the `calls:` block on a windowed `context` and `read`, its `--json` key, its source hidden once the session reads it, its absence on a whole-file read, every call named when its call sites overrun the budget, a repeated function's source shown once, one head per function with all its lines once heads per call overrun, one line per file once those overrun too with the code still printed inside `--budget`, other files' call sites before the reading file's, a trimmed window's hint continuing after its last printed line, a call used in more files than the work budget naming the function its line resolves to with its source and no `callers` count, a call defined in more files than the work budget named with its `defined_in` count and no source, a call on a base class naming its declared method beside the overrides, and the work budget resolving every cheap name before a costly one.
- `tests/enrichment.rs` pins that a warm file call times one `git status`, one `rev-parse HEAD`, and one `for-each-ref` and no `ls-files` or `show-toplevel`, and that a 300-entry directory line costs no per-file facts.
- `tests/search.rs` pins matches grouped under their declarations and the `declaration` and `type` JavaScript Object Notation fields on `grep`, `grep --at`, and `pattern`, ripgrep's flags on `grep`, several path arguments, and the one-line-per-directory listing when bare paths overrun the budget.
- `tests/git.rs` pins the `changed:`, `removed:`, `added:`, and `touches:` rows on a `diff` row and their four JavaScript Object Notation arrays.
- `tests/git.rs` pins that a partial clone answers `grep` without fetching and still fetches for `read --at`.
- `tests/git.rs` pins that `history --contains` names the enclosing declaration `grep` names and spawns no ctags.
- `tests/git.rs` pins `history --contains` showing the newest 29 commits, the line naming the commits between, and the oldest, and every commit under `--all`; leaving out merge commits, binary files, and the commits a rewrite removed; walking only a new commit; naming a removal's line from the file before it; and naming the lines a change added and removed, each with its declaration. It pins `history --commit` fitting `--budget`.
- `tests/concurrency.rs` pins that eight concurrent `history --contains` calls walk the commits once.
- `tests/architecture_commands.rs` pins the surface row on a `defines`, `usages`, and `dependencies` result and the unfabricated module row.
- `tests/session_log.rs` and `tests/concurrency.rs` pin the session directory baseline and the `status` and `diff` directory block.
- `tests/session_log.rs` pins the session-log event set and read coverage.
- `tests/enrichment.rs` pins doc delivery: a doc longer than the budget arriving in parts, every line once and in order; a doc read in part continuing after the read; a doc sent whole staying loaded after a partial read; Markdown read verbatim counting as read; several paths with `--skip`; and `--agent` keeping a Subagent's record its own.
- `tests/concurrency.rs` pins that concurrent doc deliveries for one session never send the same lines.
- `tests/concurrency.rs` pins that one Agent's parallel windowed `context` calls wait on its lock for under a tenth of their render, because each resolves its `calls:` before its first gate.
- `tests/docs_status.rs` pins that a `trace docs` or `read --docs` whose stdout closes before it prints records no doc as sent, in text and `--json`, and that `trace docs --json --filter` records none.
- `tests/concurrency.rs` holds the deadlock tripwire: a 600-source fixture whose primer runs twenty times under a ten-second deadline.
- `tests/speed.rs` contains loose regression tripwires for command latency.
- Binary-file `read` exits 0 and names the file's size instead of its bytes.
- `survey` outside a git repository exits 0 with structurally valid empty JavaScript Object Notation.
- `status` outside a git repository exits 0 with structurally valid empty JavaScript Object Notation.
- Explicit not-found errors use stderr and exit 2.

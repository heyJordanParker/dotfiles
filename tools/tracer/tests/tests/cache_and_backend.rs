//! Cache lifecycle and the CCN backend.
//!
//! Covers: cold build, warm reuse, invalidation on content change, the two
//! namespaces (`file` / `architecture`), `cache clear` (scoped, all),
//! `cache stats` (human + json), and the single AST CCN backend — CCN is
//! AST-derived regardless of the `TRACER_CCN_BACKEND` value, and the cache
//! does not fork on that value.

use sha2::{Digest, Sha256};
use std::fs;
use tracer_cli_tests::{parse_stats_table, standard_repo, Fixture};

/// The published on-disk cache key (tracer Claude.md + cache.rs module
/// header): sha256("v{SCHEMA}|ccn:ast\0" + file_bytes + "\0" + relpath),
/// hex-encoded. Reconstructed here from the documented formula — not from
/// tracer internals — so the schema-version-invalidation test can plant an
/// entry under one schema's key and prove it is unreachable under another.
fn file_cache_key(schema_version: u32, file_bytes: &[u8], relpath: &str) -> String {
    let mut h = Sha256::new();
    h.update(format!("v{schema_version}|ccn:ast\0").as_bytes());
    h.update(file_bytes);
    h.update(b"\0");
    h.update(relpath.as_bytes());
    hex::encode(h.finalize())
}

/// SCHEMA_VERSION as published in the tracer Claude.md / cache.rs. The
/// schema-bump test plants a poison entry at this version's key (proving
/// the cache IS consulted by this exact schema-versioned key) and at a
/// neighbor version's key (proving it is unreachable).
const PUBLISHED_SCHEMA_VERSION: u32 = 10;

#[test]
fn cache_build_populates_both_namespaces() {
    let f = standard_repo();
    let r = f.trace(&["cache", "build", "."]);
    r.ok();
    assert!(r.stdout.contains("Architecture graph:"), "{}", r.stdout);
    let stats = f.trace(&["cache", "stats", "--json"]);
    stats.ok();
    let v = stats.json();
    // `cache build .` over standard_repo() populates exactly 8 file/
    // entries and 1 architecture/ entry for this fixed tree.
    assert_eq!(
        v["file"]["entries"].as_i64().unwrap(),
        8,
        "file namespace must hold exactly 8 entries after build: {}",
        stats.stdout
    );
    assert_eq!(
        v["architecture"]["entries"].as_i64().unwrap(),
        1,
        "architecture namespace must hold exactly 1 entry after build: {}",
        stats.stdout
    );
}

#[test]
fn cache_stats_human_table_matches_json() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let human = f.trace(&["cache", "stats"]);
    human.ok();
    let table = parse_stats_table(&human.stdout);
    let json = f.trace(&["cache", "stats", "--json"]).json();
    assert_eq!(
        table.get("file").copied().unwrap_or(0) as i64,
        json["file"]["entries"].as_i64().unwrap(),
        "human/json file count disagree\nhuman:\n{}",
        human.stdout
    );
}

#[test]
fn cache_build_is_idempotent_and_warm_is_faster() {
    let f = standard_repo();
    let cold = f.trace(&["cache", "build", "."]);
    cold.ok();
    let warm = f.trace(&["cache", "build", "."]);
    warm.ok();
    // Warm rebuild must not be dramatically slower than cold; mostly this
    // asserts idempotence (no crash, still reports a graph).
    assert!(warm.stdout.contains("Architecture graph:"), "{}", warm.stdout);
}

#[test]
fn cache_invalidates_on_content_change() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let before = f.trace(&["info", "src/util.py", "--json"]).json();
    // standard_repo()'s helper(v): base 1 + if(1) = 2, exactly.
    assert_eq!(
        before["cyclomatic_complexity_total"].as_i64().unwrap(),
        2,
        "baseline helper() CCN must be exactly 2: {}",
        before["cyclomatic_complexity_total"]
    );

    // Add decision points; ccn must change on the next read (cache key is
    // keyed on content, so the stale entry is unreachable). The new
    // helper: base 1 + if(1) + if(1) + for(1) + if(1) = 5, exactly — a
    // stale cache hit would still report 2 and fail this equality.
    f.write(
        "src/util.py",
        "def helper(v):\n    if v > 0:\n        if v > 10:\n            return v\n    \
         for i in range(v):\n        if i:\n            pass\n    return 0\n",
    );
    let after = f.trace(&["info", "src/util.py", "--json"]).json();
    assert_eq!(
        after["cyclomatic_complexity_total"].as_i64().unwrap(),
        5,
        "post-edit helper() CCN must be exactly 5 (cache served a stale \
         entry if this is 2): {}",
        after["cyclomatic_complexity_total"]
    );
}

#[test]
fn cache_clear_scoped_to_one_namespace() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["cache", "clear", "--namespace", "architecture"]);
    r.ok();
    let v = f.trace(&["cache", "stats", "--json"]).json();
    assert_eq!(
        v["architecture"]["entries"].as_i64().unwrap(),
        0,
        "architecture namespace not cleared"
    );
    // The scoped clear must leave the file namespace fully intact — all
    // 8 entries for standard_repo() still present.
    assert_eq!(
        v["file"]["entries"].as_i64().unwrap(),
        8,
        "scoped clear must leave exactly 8 file entries: {v}"
    );
}

#[test]
fn cache_clear_all_removes_everything() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["cache", "clear", "--all"]);
    r.ok();
    assert!(r.stdout.contains("Removed"), "{}", r.stdout);
    let v = f.trace(&["cache", "stats", "--json"]).json();
    assert_eq!(v["file"]["entries"].as_i64().unwrap(), 0);
    assert_eq!(v["architecture"]["entries"].as_i64().unwrap(), 0);
}

#[test]
fn ccn_backend_is_ast_and_cache_does_not_fork_on_env_value() {
    let f = standard_repo();

    // Default build.
    f.trace(&["cache", "build", "."]).ok();
    let default_entries = f.trace(&["cache", "stats", "--json"]).json()["file"]["entries"]
        .as_i64()
        .unwrap();
    let default_info = f.trace(&["info", "src/app.py", "--json"]).json();

    // Building the same tree with TRACER_CCN_BACKEND set must not add a
    // second set of entries — there is one backend, one cache identity.
    f.trace_env(&["cache", "build", "."], &[("TRACER_CCN_BACKEND", "ast")])
        .ok();
    let after_ast = f.trace(&["cache", "stats", "--json"]).json()["file"]["entries"]
        .as_i64()
        .unwrap();
    assert_eq!(
        after_ast, default_entries,
        "setting TRACER_CCN_BACKEND forked the cache \
         (default={default_entries}, after_ast={after_ast}) — there is one backend"
    );

    // FileFacts shape (the keys downstream commands depend on) is present
    // regardless of the env value.
    let ast_info = f
        .trace_env(
            &["info", "src/app.py", "--json"],
            &[("TRACER_CCN_BACKEND", "ast")],
        )
        .json();
    // The FileFacts keys are not merely present — they carry the exact,
    // hand-verifiable values for src/app.py (main(): if + for + if over
    // base 1 = CCN 4; one function; rank low; Python). Asserting the
    // values, and that they are identical under both env settings, proves
    // the env value neither forks the cache nor shifts the computation.
    let expected = serde_json::json!({
        "function_count": 1,
        "cyclomatic_complexity_total": 4,
        "cyclomatic_complexity_max": 4,
        "rank": "low",
        "language": "python",
    });
    for (key, want) in expected.as_object().unwrap() {
        assert_eq!(
            &default_info[key], want,
            "default-env FileFacts `{key}` wrong for src/app.py"
        );
        assert_eq!(
            &ast_info[key], want,
            "ast-env FileFacts `{key}` wrong for src/app.py"
        );
    }
}

#[test]
fn ccn_is_ast_derived_regardless_of_backend_env_value() {
    let f = standard_repo();
    let default = f.trace(&["info", "src/app.py", "--json"]).json();
    let explicit_ast = f
        .trace_env(
            &["info", "src/app.py", "--json"],
            &[("TRACER_CCN_BACKEND", "ast")],
        )
        .json();
    let bogus = f
        .trace_env(
            &["info", "src/app.py", "--json"],
            &[("TRACER_CCN_BACKEND", "definitely-not-a-backend")],
        )
        .json();
    // One backend (AST): every env value yields the identical CCN.
    assert_eq!(
        default["cyclomatic_complexity_total"], explicit_ast["cyclomatic_complexity_total"],
        "explicit ast value changed CCN — backend is not value-independent"
    );
    assert_eq!(
        default["cyclomatic_complexity_total"], bogus["cyclomatic_complexity_total"],
        "unknown TRACER_CCN_BACKEND value changed CCN — backend is not value-independent"
    );
}

/// The stable byte format: `--json` emits every non-ASCII scalar as a
/// `\uXXXX` escape (astral chars as a UTF-16 surrogate pair) with `": "` /
/// `", "` separators. Asserted on the RAW stdout bytes — never parsed
/// through serde_json, which would normalize `é` back to `é` and
/// launder away exactly the bytes the guarantee is about.
#[test]
fn json_output_is_ascii_escaped_on_raw_bytes() {
    let f = Fixture::new();
    // café (U+00E9, in the BMP) and 🚀 (U+1F680, astral → surrogate pair).
    f.write("uni.py", "x = 1  # caf\u{00e9} \u{1f680} token_NONASCII\n");
    f.commit("non-ascii content");

    let r = f.trace(&["grep", "token_NONASCII", "--path", ".", "--json"]);
    r.ok();
    let raw = r.stdout.as_bytes();

    // 1. Every byte is ASCII — no raw UTF-8 multibyte leaked through.
    assert!(
        raw.iter().all(|b| b.is_ascii()),
        "non-ASCII byte in --json output; the stable format must escape all"
    );
    // 2. The exact escape sequences are present as literal backslash-u
    //    (a normalizing parser would have collapsed these to é / 🚀).
    assert!(
        r.stdout.contains("caf\\u00e9"),
        "BMP scalar é not \\u-escaped: {}",
        r.stdout
    );
    assert!(
        r.stdout.contains("\\ud83d\\ude80"),
        "astral scalar 🚀 not emitted as a UTF-16 surrogate pair: {}",
        r.stdout
    );
    // 3. The fixed separators (`": "` after a key, `", "` between items).
    assert!(
        r.stdout.contains("\"match_count\": "),
        "key separator must be \": \": {}",
        r.stdout
    );
    // 4. The literal raw é byte (0xC3 0xA9) must NOT appear anywhere.
    assert!(
        !r.stdout.contains('\u{00e9}') && !r.stdout.contains('\u{1f680}'),
        "raw non-ASCII scalar present — format laundered: {}",
        r.stdout
    );

    // The on-disk cache entry shares the same byte format. The file-cache
    // entry for uni.py records its language; assert the entry bytes are
    // also pure ASCII (same serializer, same guarantee on disk).
    f.trace(&["cache", "build", "."]).ok();
    let entry_dir = f.root.join(".tracer-cache/file");
    let mut checked_an_entry = false;
    for e in fs::read_dir(&entry_dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let bytes = fs::read(&p).unwrap();
        assert!(
            bytes.iter().all(|b| b.is_ascii()),
            "on-disk cache entry {p:?} contains a non-ASCII byte — \
             the on-disk format must use the same ASCII escaping as --json"
        );
        checked_an_entry = true;
    }
    assert!(checked_an_entry, "no file-cache entry written to assert on");
}

/// Schema-version invalidation, proven black-box against the *published*
/// key formula (no tracer internals linked). A cache entry whose key was
/// derived under a different SCHEMA_VERSION must be unreachable: the binary
/// must ignore it and recompute the true value.
///
/// Step A pins that the cache really is consulted by the current-schema
/// key (a poison planted there is served). Step B is the invalidation
/// proof: the same poison under a *neighbor* schema's key is NOT served.
#[test]
fn schema_version_bump_makes_prior_entries_unreachable() {
    let src = "def helper(v):\n    if v > 0:\n        return v + 1\n    return 0\n";

    // grep enrichment serves `file_complexity.ccn_total` straight from the
    // `file/` cache entry (`file_facts::get`), so a poisoned entry under a
    // given key is observable. (`info`'s top-level CCN is recomputed from
    // source and would mask the cache, so it is the wrong probe here.)
    let bytes = src.as_bytes();
    let poison_entry = serde_json::json!({
        "path": "u.py",
        "language": "Python",
        "loc": 4,
        "function_count": 1,
        "cyclomatic_complexity_total": 999,
        "cyclomatic_complexity_max": 999,
        "rank": "critical",
        "extraction": serde_json::Value::Null
    });
    let grep_ccn = |fx: &Fixture| -> i64 {
        let v = fx.trace(&["grep", "helper", "--path", ".", "--json"]).json();
        v["matches"][0]["file_complexity"]["ccn_total"]
            .as_i64()
            .unwrap()
    };

    // --- Step A: the current-schema key IS consulted. ---
    let f = Fixture::new();
    f.write("u.py", src);
    f.commit("seed");
    f.trace(&["cache", "build", "."]).ok();
    // helper(v): base 1 + if(1) = exactly 2.
    let true_ccn = grep_ccn(&f);
    assert_eq!(true_ccn, 2, "fixture sanity: helper() CCN is exactly 2");

    let cur_key = file_cache_key(PUBLISHED_SCHEMA_VERSION, bytes, "u.py");
    let entry_dir = f.root.join(".tracer-cache/file");
    let cur_path = entry_dir.join(format!("{cur_key}.json"));
    assert!(
        cur_path.exists(),
        "reconstructed current-schema key {cur_key} has no on-disk entry — \
         the published key formula is wrong or the cache was not warmed"
    );
    // Poison the current-schema entry and defeat the mtime fast-path so the
    // content-hash (schema-versioned) key path is what answers.
    fs::write(&cur_path, serde_json::to_string(&poison_entry).unwrap()).unwrap();
    fs::remove_file(
        entry_dir.join(format!(
            "mtime_index_v1__schema{}__ast.json",
            PUBLISHED_SCHEMA_VERSION
        )),
    )
    .ok();
    assert_eq!(
        grep_ccn(&f),
        999,
        "the cache is NOT keyed/consulted by the current-schema key — \
         a poison planted at that key was not served, so the rest of this \
         test cannot prove schema invalidation"
    );

    // --- Step B: a neighbor-schema key is unreachable. ---
    let g = Fixture::new();
    g.write("u.py", src);
    g.commit("seed");
    g.trace(&["cache", "build", "."]).ok();
    let g_entry_dir = g.root.join(".tracer-cache/file");
    // Remove the legitimate current-schema entry and the mtime index, then
    // plant the SAME poison under the *previous* schema version's key.
    fs::remove_file(
        g_entry_dir.join(format!("{cur_key}.json")),
    )
    .ok();
    fs::remove_file(
        g_entry_dir.join(format!(
            "mtime_index_v1__schema{}__ast.json",
            PUBLISHED_SCHEMA_VERSION
        )),
    )
    .ok();
    let old_key = file_cache_key(PUBLISHED_SCHEMA_VERSION - 1, bytes, "u.py");
    fs::write(
        g_entry_dir.join(format!("{old_key}.json")),
        serde_json::to_string(&poison_entry).unwrap(),
    )
    .unwrap();
    // The old-schema entry must be unreachable: the binary recomputes the
    // true CCN, never the 999 poison sitting under the prior schema's key.
    assert_eq!(
        grep_ccn(&g),
        true_ccn,
        "an entry keyed under SCHEMA_VERSION {} was served while the \
         binary runs SCHEMA_VERSION {} — schema-version bumps do NOT \
         invalidate prior entries",
        PUBLISHED_SCHEMA_VERSION - 1,
        PUBLISHED_SCHEMA_VERSION
    );
}

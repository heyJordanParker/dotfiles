//! `trace logs`: entry framing, the ignore bypass, time windows across
//! rotated files, compressed rotations, and the newest-first cap.

use tracer_cli_tests::Fixture;

/// gzip of `10.0.0.3 - - [15/Aug/2026:10:53:09 +0000] "POST /reset-theme
/// HTTP/1.1" 200 55\n`, written byte-for-byte so the fixture needs no
/// compression tool and no crate.
const ACCESS_GZ: &[u8] = &[
    31, 139, 8, 0, 0, 0, 0, 0, 0, 3, 51, 52, 208, 3, 65, 99, 5, 93, 32, 140, 54, 52, 213, 119, 44,
    77, 215, 55, 50, 48, 50, 179, 50, 52, 176, 50, 53, 182, 50, 176, 84, 208, 54, 0, 130, 88, 5,
    165, 0, 255, 224, 16, 5, 253, 162, 212, 226, 212, 18, 221, 146, 140, 212, 220, 84, 5, 143, 144,
    144, 0, 125, 67, 61, 67, 37, 5, 35, 3, 3, 5, 83, 83, 46, 0, 124, 4, 89, 27, 78, 0, 0, 0,
];

/// A repository whose logs are gitignored, which is the state every project
/// with logs is in.
fn log_repo() -> Fixture {
    let f = Fixture::new();
    f.write(".gitignore", "storage/logs/*.log\n");
    f.write("src/app.py", "def main():\n    return 1\n");
    f.write(
        "storage/logs/dent-2026-08-14.log",
        concat!(
            "[2026-08-14 21:00:00] production.INFO: evening run\n",
            "[2026-08-14 22:30:00] production.ERROR: reset-theme failed\n",
            "[2026-08-14 23:59:00] production.INFO: midnight approach\n",
        ),
    );
    f.write(
        "storage/logs/dent-2026-08-15.log",
        concat!(
            "[2026-08-15 10:51:00] production.INFO: cache warmed\n",
            "[2026-08-15 10:52:01] production.ERROR: reset-theme failed\n",
            "#0 /app/Http/Controllers/ThemeController.php(41): reset()\n",
            "#1 /app/Kernel.php(12): handle()\n",
            "[2026-08-15 10:53:00] production.INFO: retry scheduled\n",
            "[2026-08-15 11:10:00] production.INFO: settled\n",
        ),
    );
    f.commit("init log repo");
    f
}

fn entries(v: &serde_json::Value) -> &Vec<serde_json::Value> {
    v["entries"].as_array().expect("entries array")
}

/// The reproduction: `grep` cannot see a gitignored log at all, so the
/// whole command exists for this one assertion.
#[test]
fn a_gitignored_log_is_searchable() {
    let f = log_repo();
    let ignored = f.trace(&["grep", "reset-theme", "--path", "storage/logs", "--json"]);
    ignored.ok();
    assert_eq!(
        ignored.json()["match_count"].as_i64().unwrap(),
        0,
        "grep is expected to be blind here — that is why logs exists"
    );

    let r = f.trace(&["logs", "reset-theme", "--path", "storage/logs", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["entry_count"].as_i64().unwrap(), 2, "{v}");
}

#[test]
fn a_stack_trace_returns_as_one_entry() {
    let f = log_repo();
    let r = f.trace(&[
        "logs",
        "reset-theme",
        "--path",
        "storage/logs/dent-2026-08-15.log",
        "--json",
    ]);
    r.ok();
    let v = r.json();
    let e = entries(&v);
    assert_eq!(e.len(), 1, "{v}");
    assert_eq!(e[0]["line"].as_i64().unwrap(), 2);
    assert_eq!(e[0]["stamp"].as_str().unwrap(), "2026-08-15 10:52:01");
    let text = e[0]["text"].as_str().unwrap();
    assert_eq!(text.lines().count(), 3, "the two frames belong to the entry");
    assert!(text.contains("#1 /app/Kernel.php(12): handle()"), "{text}");
}

#[test]
fn every_line_of_a_stamped_log_is_its_own_entry() {
    let f = log_repo();
    let r = f.trace(&[
        "logs",
        "--path",
        "storage/logs/dent-2026-08-14.log",
        "--json",
    ]);
    r.ok();
    let v = r.json();
    assert_eq!(entries(&v).len(), 3, "{v}");
}

#[test]
fn a_window_spans_the_rotated_files_it_covers() {
    let f = log_repo();
    let r = f.trace(&[
        "logs",
        "--path",
        "storage/logs",
        "--since",
        "2026-08-14 22:00",
        "--until",
        "2026-08-15 10:52",
        "--json",
    ]);
    r.ok();
    let v = r.json();
    let e = entries(&v);
    let stamps: Vec<&str> = e.iter().map(|x| x["stamp"].as_str().unwrap()).collect();
    assert_eq!(
        stamps,
        vec![
            "2026-08-14 22:30:00",
            "2026-08-14 23:59:00",
            "2026-08-15 10:51:00",
            "2026-08-15 10:52:01",
        ],
        "{v}"
    );
}

#[test]
fn an_access_log_frames_on_the_date_after_the_client_address() {
    let f = Fixture::new();
    f.write(
        "access.log",
        concat!(
            "10.0.0.1 - - [15/Aug/2026:10:52:01 +0000] \"GET /theme HTTP/1.1\" 500 120\n",
            "10.0.0.2 - - [15/Aug/2026:10:52:04 +0000] \"GET /health HTTP/1.1\" 200 12\n",
        ),
    );
    let r = f.trace(&["logs", "--path", "access.log", "--json"]);
    r.ok();
    let v = r.json();
    let e = entries(&v);
    assert_eq!(e.len(), 2, "{v}");
    assert_eq!(e[0]["stamp"].as_str().unwrap(), "2026-08-15 10:52:01");
}

/// PHP's `error_log` shape, which a WordPress `debug.log` carries, opening
/// with the NUL run a concurrent writer leaves behind.
#[test]
fn a_php_log_behind_a_nul_run_is_read() {
    let f = Fixture::new();
    let mut bytes = vec![0u8; 4096];
    bytes.extend_from_slice(
        concat!(
            "[06-Jun-2026 12:52:27 UTC] WordPress database error SQLSTATE[42P01]\n",
            "LINE 1: DELETE FROM visitor_deadlines WHERE visitor_id IN ( SELECT i...\n",
            "[06-Jun-2026 12:53:00 UTC] PHP Notice: undefined index\n",
        )
        .as_bytes(),
    );
    f.write_bytes("debug.log", &bytes);
    let r = f.trace(&["logs", "--path", "debug.log", "--json"]);
    r.ok();
    let v = r.json();
    let e = entries(&v);
    assert_eq!(e.len(), 2, "{v}");
    assert_eq!(e[0]["stamp"].as_str().unwrap(), "2026-06-06 12:52:27");
    assert_eq!(
        e[0]["text"].as_str().unwrap().lines().count(),
        2,
        "the LINE 1: continuation belongs to the entry"
    );
}

#[test]
fn a_json_lines_log_frames_one_object_per_line() {
    let f = Fixture::new();
    f.write(
        "worker.log",
        concat!(
            "{\"time\":\"2026-08-15T10:52:01Z\",\"level\":\"error\",\"msg\":\"reset failed\"}\n",
            "{\"level\":\"info\",\"time\":\"2026-08-15T10:52:05Z\",\"msg\":\"retry queued\"}\n",
        ),
    );
    let r = f.trace(&["logs", "--path", "worker.log", "--json"]);
    r.ok();
    let v = r.json();
    let e = entries(&v);
    assert_eq!(e.len(), 2, "{v}");
    assert_eq!(e[1]["stamp"].as_str().unwrap(), "2026-08-15 10:52:05");
}

#[test]
fn a_log_with_no_timestamp_returns_one_entry_per_line() {
    let f = Fixture::new();
    f.write(
        "build.log",
        "compiling one\ncompiling two\nerror: target missing\n",
    );
    let r = f.trace(&["logs", "--path", "build.log", "--json"]);
    r.ok();
    let v = r.json();
    let e = entries(&v);
    assert_eq!(e.len(), 3, "{v}");
    assert!(e[0]["stamp"].is_null(), "{v}");
}

#[test]
fn a_compressed_rotation_is_read() {
    let f = Fixture::new();
    f.write_bytes("access.log.2.gz", ACCESS_GZ);
    let r = f.trace(&["logs", "reset-theme", "--path", ".", "--json"]);
    r.ok();
    let v = r.json();
    let e = entries(&v);
    assert_eq!(e.len(), 1, "{v}");
    assert_eq!(e[0]["stamp"].as_str().unwrap(), "2026-08-15 10:53:09");
}

#[test]
fn the_cap_keeps_the_newest_entries() {
    let f = log_repo();
    let r = f.trace(&["logs", "--path", "storage/logs", "--limit", "2", "--json"]);
    r.ok();
    let v = r.json();
    let e = entries(&v);
    let stamps: Vec<&str> = e.iter().map(|x| x["stamp"].as_str().unwrap()).collect();
    assert_eq!(
        stamps,
        vec!["2026-08-15 10:53:00", "2026-08-15 11:10:00"],
        "{v}"
    );
}

#[test]
fn around_returns_whole_neighbouring_entries() {
    let f = log_repo();
    let r = f.trace(&[
        "logs",
        "reset-theme",
        "--path",
        "storage/logs/dent-2026-08-15.log",
        "--around",
        "1",
        "--json",
    ]);
    r.ok();
    let v = r.json();
    let stamps: Vec<&str> = entries(&v)
        .iter()
        .map(|x| x["stamp"].as_str().unwrap())
        .collect();
    assert_eq!(
        stamps,
        vec![
            "2026-08-15 10:51:00",
            "2026-08-15 10:52:01",
            "2026-08-15 10:53:00",
        ],
        "{v}"
    );
}

#[test]
fn a_malformed_window_exits_two() {
    let f = log_repo();
    let r = f.trace(&["logs", "--path", "storage/logs", "--since", "yesterday"]);
    r.code_is(2);
    assert!(r.stderr.contains("YYYY-MM-DD"), "{}", r.stderr);
}

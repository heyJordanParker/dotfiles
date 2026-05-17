//! `trace survey` — repo-wide complexity distribution via scc.
//! One `scc --format json --by-file <path>` sweep → per-language
//! LOC/complexity aggregates, a per-file complexity distribution
//! (median/p75/p90/p95/max), and the top-10 most-complex files.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// scc failure → stderr + exit 1 (hard-fail contract).
fn scc_by_file(path: &Path) -> Vec<Value> {
    let out = match Command::new("scc")
        .args(["--format", "json", "--by-file"])
        .arg(path)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: scc failed: {e}");
            std::process::exit(1);
        }
    };
    if !out.status.success() {
        eprintln!(
            "Error: scc failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        std::process::exit(1);
    }
    serde_json::from_slice(&out.stdout).unwrap_or_default()
}

fn i64_field(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
}

/// `int(statistics.median(values))` — average of the two middles for even
/// counts, truncated toward zero. `complexities` need not be sorted.
fn median_int(complexities: &[i64]) -> i64 {
    let mut s = complexities.to_vec();
    s.sort_unstable();
    let n = s.len();
    if n == 0 {
        return 0;
    }
    if n % 2 == 1 {
        s[n / 2]
    } else {
        ((s[n / 2 - 1] + s[n / 2]) as f64 / 2.0) as i64
    }
}

/// Builds the languages map, per-file list, distribution, and top-10.
fn summary(by_file: &[Value]) -> Value {
    let mut languages = serde_json::Map::new();
    let mut files: Vec<Value> = Vec::new();

    for lang_block in by_file {
        let lang = lang_block
            .get("Name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        languages.insert(
            lang,
            json!({
                "files": i64_field(lang_block, "Count"),
                "loc": i64_field(lang_block, "Code"),
                "complexity": i64_field(lang_block, "Complexity"),
            }),
        );
        if let Some(fs) = lang_block.get("Files").and_then(|f| f.as_array()) {
            for f in fs {
                files.push(json!({
                    "path": f.get("Location").and_then(|x| x.as_str()).unwrap_or(""),
                    "language": f.get("Language").and_then(|x| x.as_str()).unwrap_or(""),
                    "loc": i64_field(f, "Code"),
                    "complexity": i64_field(f, "Complexity"),
                }));
            }
        }
    }

    let complexities: Vec<i64> = files
        .iter()
        .map(|f| f.get("complexity").and_then(|x| x.as_i64()).unwrap_or(0))
        .collect();

    if complexities.is_empty() {
        return json!({
            "languages": Value::Object(languages),
            "files": [],
            "distribution": {},
            "top_complex": [],
        });
    }

    let mut sorted_c = complexities.clone();
    sorted_c.sort_unstable();
    let n = sorted_c.len();
    // Percentile index: max(0, floor(n * p) - 1).
    let pct = |p: f64| -> i64 {
        let idx = (((n as f64) * p) as i64 - 1).max(0) as usize;
        sorted_c[idx]
    };

    let distribution = json!({
        "median": median_int(&complexities),
        "p75": pct(0.75),
        "p90": pct(0.90),
        "p95": pct(0.95),
        "max": sorted_c[n - 1],
    });

    let mut top = files.clone();
    // Top 10 by complexity descending, stable sort.
    top.sort_by(|a, b| {
        b.get("complexity")
            .and_then(|x| x.as_i64())
            .unwrap_or(0)
            .cmp(&a.get("complexity").and_then(|x| x.as_i64()).unwrap_or(0))
    });
    top.truncate(10);

    json!({
        "total_files": n,
        "languages": Value::Object(languages),
        "distribution": distribution,
        "top_complex": top,
    })
}

pub fn run(path: &Path, as_json: bool) -> Result<Value> {
    crate::pathval::require_exists(path, "PATH");
    let abs = crate::cache::absolutize(path);
    let resolved = abs.canonicalize().unwrap_or(abs);
    let by_file = scc_by_file(&resolved);
    let s = summary(&by_file);

    if as_json {
        return Ok(s);
    }

    println!(
        "Files: {}",
        s.get("total_files").and_then(|x| x.as_i64()).unwrap_or(0)
    );
    println!();
    println!("Languages:");
    let mut langs: Vec<(&String, &Value)> = s["languages"]
        .as_object()
        .map(|m| m.iter().collect())
        .unwrap_or_default();
    // Top 15 languages by loc descending, stable sort.
    langs.sort_by(|a, b| {
        b.1.get("loc")
            .and_then(|x| x.as_i64())
            .unwrap_or(0)
            .cmp(&a.1.get("loc").and_then(|x| x.as_i64()).unwrap_or(0))
    });
    for (lang, stats) in langs.iter().take(15) {
        println!(
            "  {:<20} files={:<6} loc={:<8} complexity={}",
            lang,
            stats.get("files").and_then(|x| x.as_i64()).unwrap_or(0),
            stats.get("loc").and_then(|x| x.as_i64()).unwrap_or(0),
            stats.get("complexity").and_then(|x| x.as_i64()).unwrap_or(0),
        );
    }
    println!();
    let dist = &s["distribution"];
    if dist.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        println!("Complexity distribution (per file):");
        println!(
            "  median={}  p75={}  p90={}  p95={}  max={}",
            dist["median"].as_i64().unwrap_or(0),
            dist["p75"].as_i64().unwrap_or(0),
            dist["p90"].as_i64().unwrap_or(0),
            dist["p95"].as_i64().unwrap_or(0),
            dist["max"].as_i64().unwrap_or(0),
        );
    }
    println!();
    println!("Top 10 most-complex files:");
    if let Some(top) = s["top_complex"].as_array() {
        for f in top {
            println!(
                "  {:>5}  {:>5} loc  {}",
                f.get("complexity").and_then(|x| x.as_i64()).unwrap_or(0),
                f.get("loc").and_then(|x| x.as_i64()).unwrap_or(0),
                f.get("path").and_then(|x| x.as_str()).unwrap_or(""),
            );
        }
    }
    Ok(s)
}

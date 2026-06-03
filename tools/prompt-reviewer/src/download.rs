//! Download the fixed model GGUF to its on-disk path, once, with resume.
//!
//! The file is large (~5.3 GB) and is never committed or downloaded by machine
//! setup. This is the only place that writes it. A partial download resumes
//! from the bytes already on disk via an HTTP Range request.

use anyhow::{bail, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use crate::model;

/// Download the GGUF to `model::path()`, resuming a partial file if present.
/// No-op with a clear message when the full file is already there.
pub fn run() -> Result<()> {
    let dest = model::path();
    let url = model::download_url();

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating cache directory {}", parent.display()))?;
    }

    let total = remote_size(&url)?;
    let have = dest.metadata().map(|m| m.len()).unwrap_or(0);

    if have == total {
        println!("Model already present: {} ({} bytes)", dest.display(), total);
        return Ok(());
    }
    if have > total {
        bail!(
            "On-disk file {} is larger ({have} bytes) than the remote ({total} bytes); \
             delete it and download again",
            dest.display()
        );
    }

    println!(
        "Downloading {} -> {}\n  {} of {} bytes already present",
        model::GGUF_FILE,
        dest.display(),
        have,
        total
    );

    let request = ureq::get(&url).set("Range", &format!("bytes={have}-"));
    let response = request.call().context("requesting the model file")?;
    let status = response.status();
    if have > 0 && status != 206 {
        bail!("server did not honor resume (HTTP {status}); delete the partial file and retry");
    }
    if have == 0 && !(200..300).contains(&status) {
        bail!("unexpected HTTP {status} downloading the model");
    }

    let mut reader = response.into_reader();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&dest)
        .with_context(|| format!("opening {} for writing", dest.display()))?;

    copy_with_progress(&mut reader, &mut file, have, total)?;

    let final_size = dest.metadata()?.len();
    if final_size != total {
        bail!("download ended at {final_size} bytes, expected {total}; re-run to resume");
    }
    println!("\nDone: {} ({} bytes)", dest.display(), total);
    Ok(())
}

/// HEAD the URL for the full content length (Content-Range total when the
/// origin only answers ranged requests, else Content-Length).
fn remote_size(url: &str) -> Result<u64> {
    let response = ureq::head(url).call().context("HEAD request for size")?;
    if let Some(len) = response.header("content-length") {
        if let Ok(n) = len.parse::<u64>() {
            if n > 0 {
                return Ok(n);
            }
        }
    }
    bail!("could not determine model size from the server");
}

fn copy_with_progress(
    reader: &mut impl Read,
    file: &mut File,
    start: u64,
    total: u64,
) -> Result<()> {
    let mut buffer = vec![0u8; 1 << 20];
    let mut written = start;
    let mut last_report = start;
    loop {
        let n = reader.read(&mut buffer).context("reading download stream")?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n]).context("writing model file")?;
        written += n as u64;
        if written - last_report >= 64 << 20 {
            let percent = (written as f64 / total as f64) * 100.0;
            print!("\r  {written} / {total} bytes ({percent:.1}%)");
            std::io::stdout().flush().ok();
            last_report = written;
        }
    }
    Ok(())
}

/// Human-facing instructions for obtaining the model — printed by `doctor`
/// and surfaced whenever the model is required but absent.
pub fn download_instructions() -> String {
    let dest = model::path();
    format!(
        "The model is not present. Download it once with:\n\
         \n    review-prompt download\n\n\
         It downloads {} ({}, ~5.3 GB) to:\n    {}\n\n\
         Override the location with the {} environment variable.",
        model::GGUF_FILE,
        model::REPO,
        dest.display(),
        model::PATH_ENV
    )
}

/// True when the full model file is on disk (size-checked is overkill here;
/// presence is the contract `doctor` reports and `review` requires).
pub fn is_present(path: &Path) -> bool {
    path.is_file()
}

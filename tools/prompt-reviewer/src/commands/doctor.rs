//! `review-prompt doctor` — report whether the fixed model is present.
//!
//! Prints the model identity and its on-disk path. When the file is there,
//! reports its size and exits 0. When absent, prints how to download it and
//! exits 1 — the same fail-loud shape `trace doctor` uses for a missing
//! dependency.

use anyhow::Result;

use crate::{download, model};

pub fn run() -> Result<()> {
    let path = model::path();
    println!("Model:    {}", model::IDENTITY);
    println!("Base:     {}", model::BASE_MODEL);
    println!("Source:   {}", model::REPO);
    println!("Path:     {}", path.display());
    println!();

    if download::is_present(&path) {
        let size = path.metadata()?.len();
        println!("\u{2713} Present ({size} bytes)");
        return Ok(());
    }

    println!("\u{2717} Not present");
    println!();
    println!("{}", download::download_instructions());
    std::process::exit(1);
}

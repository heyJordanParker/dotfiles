//! `review-prompt download` — download the fixed model GGUF on demand, with
//! resume. The model is never committed or downloaded by machine setup; this
//! is how a user obtains it once.

use anyhow::Result;

use crate::download;

pub fn run() -> Result<()> {
    download::run()
}

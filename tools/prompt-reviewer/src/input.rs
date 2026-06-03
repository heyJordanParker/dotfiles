//! Resolve the two review inputs from their CLI flags.
//!
//! Real prompts are large and multi-line, so the prompt under review can be
//! supplied inline (positional), from a file (`--file`), or from stdin
//! (`--stdin`). The review instructions can be supplied inline
//! (`--instructions`) or from a file (`--instructions-file`). stdin is one
//! stream and it feeds one input — the prompt — so instructions have no stdin
//! form. Exactly one source is set per input.

use std::io::Read;

use anyhow::{bail, Context, Result};

/// One input's resolved source.
pub enum Source {
    Inline(String),
    File(String),
    Stdin,
}

impl Source {
    /// Resolve the prompt under review from its three flags, or fail if
    /// none/several are set. stdin feeds the prompt and only the prompt.
    pub fn from_prompt_flags(
        inline: Option<String>,
        file: Option<String>,
        stdin: bool,
    ) -> Result<Source> {
        match (inline, file, stdin) {
            (Some(text), None, false) => Ok(Source::Inline(text)),
            (None, Some(path), false) => Ok(Source::File(path)),
            (None, None, true) => Ok(Source::Stdin),
            (None, None, false) => bail!(
                "no prompt given: pass it as the positional argument, with --file, or --stdin"
            ),
            _ => bail!("give the prompt exactly one way: the positional argument, --file, or --stdin"),
        }
    }

    /// Resolve the review instructions from their two flags, or fail if
    /// none/both are set. Instructions have no stdin form.
    pub fn from_instructions_flags(
        inline: Option<String>,
        file: Option<String>,
    ) -> Result<Source> {
        match (inline, file) {
            (Some(text), None) => Ok(Source::Inline(text)),
            (None, Some(path)) => Ok(Source::File(path)),
            (None, None) => {
                bail!("no instructions given: pass them with --instructions or --instructions-file")
            }
            (Some(_), Some(_)) => {
                bail!("give the instructions exactly one way: --instructions or --instructions-file")
            }
        }
    }

    /// Read the text. Stdin is drained to end; a file is read whole.
    pub fn read(self) -> Result<String> {
        match self {
            Source::Inline(text) => Ok(text),
            Source::File(path) => {
                std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))
            }
            Source::Stdin => {
                let mut buffer = String::new();
                std::io::stdin()
                    .read_to_string(&mut buffer)
                    .context("reading stdin")?;
                Ok(buffer)
            }
        }
    }
}

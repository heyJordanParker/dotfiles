//! The one fixed model this tool reviews with, and where its file lives.
//!
//! The identity is hardcoded and unswappable: Google's stock Gemma 4 E4B
//! instruct release, in the llama.cpp project's own GGUF conversion
//! (`ggml-org/gemma-4-E4B-it-GGUF`). No flag, no env var, swaps the model;
//! only the *path* to the cached file is overridable (a machine detail, not
//! an identity choice).

use std::path::PathBuf;

/// Hugging Face repo holding the GGUF. The ggml-org conversion is the
/// llama.cpp project's own, faithful to the stock Google instruct weights.
pub const REPO: &str = "ggml-org/gemma-4-E4B-it-GGUF";

/// The single quantization this tool runs: Q4_K_M, the recommended
/// Apple-Silicon balance of quality and size (~5.3 GB).
pub const GGUF_FILE: &str = "gemma-4-E4B-it-Q4_K_M.gguf";

/// The upstream instruction-tuned model the GGUF is converted from. Carried
/// in output metadata so a reader knows exactly which weights produced a
/// review.
pub const BASE_MODEL: &str = "google/gemma-4-E4B-it";

/// Stable identity string stamped into every review's metadata.
pub const IDENTITY: &str = "gemma-4-E4B-it (Q4_K_M, ggml-org GGUF)";

/// Environment variable overriding the on-disk model path. Identity is fixed;
/// only where the file sits on this machine is the user's to set.
pub const PATH_ENV: &str = "PROMPT_REVIEWER_MODEL";

/// The direct-download URL for the GGUF from Hugging Face.
pub fn download_url() -> String {
    format!("https://huggingface.co/{REPO}/resolve/main/{GGUF_FILE}")
}

/// Where the GGUF lives on this machine: `$PROMPT_REVIEWER_MODEL` if set,
/// else `$XDG_CACHE_HOME/prompt-reviewer/<file>`, else
/// `$HOME/.cache/prompt-reviewer/<file>` (the macOS default cache root is
/// `~/Library/Caches`, but XDG_CACHE_HOME is honored when present).
pub fn path() -> PathBuf {
    if let Some(explicit) = std::env::var_os(PATH_ENV) {
        return PathBuf::from(explicit);
    }
    cache_dir().join("prompt-reviewer").join(GGUF_FILE)
}

fn cache_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(xdg);
    }
    let home = std::env::var_os("HOME").expect("HOME must be set");
    if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library").join("Caches")
    } else {
        PathBuf::from(home).join(".cache")
    }
}

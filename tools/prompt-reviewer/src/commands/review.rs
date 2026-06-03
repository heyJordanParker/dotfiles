//! `review-prompt` — review one prompt against free-form instructions.
//!
//! Builds the JSON envelope that is this tool's contract: the model's review
//! text verbatim plus run metadata (fixed model identity, token counts,
//! wall-clock duration). The tool neither parses nor schema-validates the
//! review content — whatever the model said is carried through as-is.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::inference::{self, Settings};
use crate::input::Source;
use crate::{download, model, output};

#[allow(clippy::too_many_arguments)]
pub fn run(
    prompt: Source,
    instructions: Source,
    max_output_tokens: i32,
    temperature: f32,
    context_size: u32,
    gpu_layers: u32,
) -> Result<()> {
    let model_path = model::path();
    if !download::is_present(&model_path) {
        bail!("{}", download::download_instructions());
    }

    let prompt_text = prompt.read()?;
    let instructions_text = instructions.read()?;

    let settings = Settings {
        max_output_tokens,
        temperature,
        context_size,
        gpu_layers,
    };
    let review = inference::run(&model_path, &instructions_text, &prompt_text, &settings)?;

    let envelope: Value = json!({
        "review": review.text,
        "model": {
            "identity": model::IDENTITY,
            "base_model": model::BASE_MODEL,
            "repo": model::REPO,
            "file": model::GGUF_FILE,
        },
        "run": {
            "prompt_tokens": review.prompt_tokens,
            "completion_tokens": review.completion_tokens,
            "duration_ms": review.duration_ms,
            "context_size": review.context_size,
            "prompt_truncated": review.prompt_truncated,
            "temperature": temperature,
            "max_output_tokens": max_output_tokens,
            "deterministic": temperature <= 0.0,
        },
    });

    output::emit(&envelope)
}

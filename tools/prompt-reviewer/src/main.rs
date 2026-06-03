//! prompt-reviewer — local prompt-review CLI. Binary name: `review-prompt`.
//!
//! Reviews a prompt against free-form instructions using one fixed, in-process
//! model (Gemma 4 E4B-it, Q4_K_M GGUF, run through llama.cpp's Rust binding
//! with GPU offload). The model identity is hardcoded and unswappable; only
//! non-identity knobs (output budget, temperature, context size, GPU offload,
//! the model file path) are configurable.

mod chat_template;
mod commands;
mod download;
mod inference;
mod input;
mod model;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand};

use input::Source;

/// Reviewing is the default action: with no subcommand, the top-level flags
/// run a review directly. `doctor` and `download` are the only subcommands.
#[derive(Parser)]
#[command(
    name = "review-prompt",
    version,
    about = "Review a prompt against free-form instructions, using one fixed local model.",
    arg_required_else_help = true
)]
struct Cli {
    /// The prompt under review, inline.
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,
    /// Read the prompt under review from a file.
    #[arg(long = "file", value_name = "PATH")]
    file: Option<String>,
    /// Read the prompt under review from stdin.
    #[arg(long = "stdin")]
    stdin: bool,

    /// The review instructions, inline.
    #[arg(long = "instructions", value_name = "TEXT")]
    instructions: Option<String>,
    /// Read the review instructions from a file.
    #[arg(long = "instructions-file", value_name = "PATH")]
    instructions_file: Option<String>,

    /// Maximum tokens the review may run to.
    #[arg(long, default_value_t = 1024)]
    max_output_tokens: i32,
    /// Sampling temperature; 0 is deterministic greedy decoding (default).
    #[arg(long, default_value_t = 0.0)]
    temperature: f32,
    /// Context window size in tokens (capped at the model's trained size).
    #[arg(long, default_value_t = 8192)]
    context_size: u32,
    /// Layers to offload to the GPU (high value offloads all available).
    #[arg(long, default_value_t = 1000)]
    gpu_layers: u32,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Report whether the fixed model is present; print how to download it if not.
    Doctor,
    /// Download the fixed model on demand, with resume.
    Download,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Doctor) => commands::doctor::run(),
        Some(Command::Download) => commands::download::run(),
        None => {
            let prompt = Source::from_prompt_flags(cli.prompt, cli.file, cli.stdin)?;
            let instructions =
                Source::from_instructions_flags(cli.instructions, cli.instructions_file)?;
            commands::review::run(
                prompt,
                instructions,
                cli.max_output_tokens,
                cli.temperature,
                cli.context_size,
                cli.gpu_layers,
            )
        }
    }
}

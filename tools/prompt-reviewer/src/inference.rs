//! Load the fixed GGUF and run one deterministic completion.
//!
//! Determinism comes from greedy sampling (argmax at every step) — identical
//! inputs at default settings produce identical output, with no seed to
//! diverge on. The model's own chat template renders the messages, so the
//! prompt the model sees is exactly the format it was trained on. Output is
//! bounded by `max_output_tokens` and by the model's end-of-turn token, so a
//! review never runs past its own turn.

use std::num::NonZeroU32;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::openai::OpenAIChatTemplateParams;
use llama_cpp_2::sampling::LlamaSampler;

/// Non-identity knobs. The model identity is fixed elsewhere; these shape a
/// single run. Defaults live as clap defaults in `main.rs` — the one source.
pub struct Settings {
    pub max_output_tokens: i32,
    pub temperature: f32,
    pub context_size: u32,
    pub gpu_layers: u32,
}

/// One model run's result: the model's raw review text plus run metadata.
pub struct Review {
    pub text: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub duration_ms: u128,
    pub context_size: u32,
    pub prompt_truncated: bool,
}

/// Render the chat messages with the model's template, then generate one
/// completion. Loads the GGUF at `model_path`, offloads to GPU per
/// `settings.gpu_layers`, and stops at `max_output_tokens` or the model's
/// end-of-turn token.
pub fn run(
    model_path: &Path,
    instructions: &str,
    prompt_under_review: &str,
    settings: &Settings,
) -> Result<Review> {
    let started = Instant::now();

    let backend = LlamaBackend::init().context("initializing the llama.cpp backend")?;
    let model_params = LlamaModelParams::default().with_n_gpu_layers(settings.gpu_layers);
    let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
        .with_context(|| format!("loading the model from {}", model_path.display()))?;

    let template = model
        .chat_template(None)
        .context("reading the model's embedded chat template")?;
    let messages = crate::chat_template::messages_json(instructions, prompt_under_review)?;
    // Render with the model's own Jinja chat template (minja) rather than
    // llama.cpp's built-in template matcher — Gemma 4's template is too new
    // for the matcher and trips its FFI path. `add_bos` here prepends BOS, so
    // tokenization below must not add it again.
    let params = OpenAIChatTemplateParams {
        messages_json: &messages,
        tools_json: None,
        tool_choice: None,
        json_schema: None,
        grammar: None,
        reasoning_format: None,
        chat_template_kwargs: None,
        add_generation_prompt: true,
        use_jinja: true,
        parallel_tool_calls: false,
        enable_thinking: false,
        add_bos: true,
        add_eos: false,
        parse_tool_calls: false,
    };
    let rendered = model
        .apply_chat_template_oaicompat(&template, &params)
        .context("rendering the chat template")?
        .prompt;

    let context_size = settings.context_size.min(model.n_ctx_train());
    let mut tokens = model
        .str_to_token(&rendered, AddBos::Never)
        .context("tokenizing the prompt")?;

    // Leave room for the completion: the prompt must fit the context window
    // with at least `max_output_tokens` to spare. When it doesn't, truncate
    // the prompt explicitly (keeping the tail, which holds the subject and
    // the instructions) rather than letting decode fail or silently clip.
    let budget = context_size as usize;
    let reserve = settings.max_output_tokens.max(1) as usize;
    let max_prompt = budget.saturating_sub(reserve).max(1);
    let prompt_truncated = tokens.len() > max_prompt;
    if prompt_truncated {
        let drop = tokens.len() - max_prompt;
        tokens.drain(0..drop);
    }
    let prompt_tokens = tokens.len();

    let mut ctx = model
        .new_context(
            &backend,
            LlamaContextParams::default()
                .with_n_ctx(Some(
                    NonZeroU32::new(context_size).context("context size must be non-zero")?,
                ))
                // Size the batch to the whole context so a full prompt prefill
                // fits in one decode — the prompt is already capped at the
                // context window above, so this is its upper bound.
                .with_n_batch(context_size),
        )
        .context("creating the inference context")?;

    let mut batch = LlamaBatch::new(prompt_tokens.max(512), 1);
    let last = prompt_tokens - 1;
    for (i, token) in tokens.iter().enumerate() {
        batch.add(*token, i as i32, &[0], i == last)?;
    }
    ctx.decode(&mut batch).context("decoding the prompt")?;

    let mut sampler = if settings.temperature <= 0.0 {
        LlamaSampler::greedy()
    } else {
        LlamaSampler::chain_simple([
            LlamaSampler::temp(settings.temperature),
            LlamaSampler::dist(0),
        ])
    };

    let mut text = String::new();
    let mut completion_tokens = 0usize;
    let mut position = prompt_tokens as i32;
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    while completion_tokens < settings.max_output_tokens as usize {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.is_eog_token(token) {
            break;
        }
        let piece = model
            .token_to_piece(token, &mut decoder, false, None)
            .context("decoding a generated token")?;
        text.push_str(&piece);
        completion_tokens += 1;

        batch.clear();
        batch.add(token, position, &[0], true)?;
        position += 1;
        ctx.decode(&mut batch).context("decoding a generated token")?;
    }

    Ok(Review {
        text: text.trim().to_string(),
        prompt_tokens,
        completion_tokens,
        duration_ms: started.elapsed().as_millis(),
        context_size,
        prompt_truncated,
    })
}

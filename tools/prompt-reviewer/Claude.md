# WHY

Small local prompt-review binary that reviews a Prompt against user-supplied review instructions with one fixed in-process model.

# Facts

- The Rust package is named `prompt-reviewer`.
- The binary is named `review-prompt`.
- Reviewing is the default action when no subcommand is provided.
- The only subcommands are `doctor` and `download`.
- Prompt input comes from a positional argument, `--file`, or `--stdin`.
- Review instructions come from `--instructions` or `--instructions-file`.
- A review emits formatted JavaScript Object Notation with `review`, `model`, and `run` fields.
- `doctor` and `download` print human-readable output.
- `src/model.rs` owns the fixed model identity.
- The fixed model repository is `ggml-org/gemma-4-E4B-it-GGUF`.
- The fixed model file is `gemma-4-E4B-it-Q4_K_M.gguf`.
- The fixed base model is `google/gemma-4-E4B-it`.
- `PROMPT_REVIEWER_MODEL` overrides only the on-disk model path.
- The tool links llama.cpp through the `llama-cpp-2` Rust crate.
- `setup.sh` builds the release binary and installs it to `~/.local/bin/review-prompt`.
- The model is downloaded by `review-prompt download`, not by `setup.sh`.
- The black-box test package is `tools/prompt-reviewer/tests`.
- The black-box tests drive the built binary through `REVIEW_PROMPT_BIN`.

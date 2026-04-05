# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build          # build the project
cargo run            # run the CLI
cargo check          # fast type/compile check without linking
PATH=/opt/homebrew/bin:$PATH cargo clippy   # lint (rustup not on PATH by default on this machine)
```

## Project Goal

A Rust CLI tool that sends manga/comic images to the Torii Translate API and saves the translated results locally. The app is fully interactive — it prompts the user through configuration and file selection before making any API calls.

## Intended Behavior

1. **API key** — prompt once (masked input), persist to `~/.torii-translate/config` with `0o600` permissions. Skip prompt if the key already exists.
2. **Translator model** — interactive selection menu:
   - Gemini 2.5 Flash Lite (1 credit) → `gemini-2.5-flash`
   - Deepseek (1 credit) → `deepseek`
   - Grok 4.1 Fast (1 credit) → `grok-4-fast`
   - Kimi K2.5 (2 credits) → `kimi-k2`
   - GPT 5.1 (2+ credits) → `gpt-5`
   - Gemini 3 Flash (2+ credits) → `gemini-3-flash`
3. **Single vs. batch** — user chooses one image or a range (e.g. `001.png`–`003.png` calls the API three times).
4. **Single image** — prompts for filename; displays the current directory name in the prompt.
5. **Batch images** — prompts for start and end filename; derives the file list by incrementing the zero-padded numeric stem. Validates that both filenames share the same extension and the same stem width (padding). Images are processed one at a time in order.
6. **Output** — the output extension is derived from the API response data URI (`data:image/png;base64,...`), not the input filename. Written to `./translated-result/<stem>.<response-ext>`.
7. **Batch interruption** — pressing Ctrl+C during a batch finishes the current file then stops cleanly, printing a summary of how many files completed.

## API

Full documentation: https://toriitranslate.com/api

### Translate endpoint (used by this tool)

`POST https://api.toriitranslate.com/api/v2/upload` — multipart form upload.
Auth: `Authorization: Bearer <api-key>`. Credits: 1+ per request (varies by model).

Required fields: `target_lang`, `translator`, `font`, `file`.
Optional fields: `text_align`, `stroke_disabled`, `min_font_size`, `custom_prompt` (max 500 chars), `context` (max 10,000 chars).

Response JSON contains `image` (translated, base64 data URI) and `inpainted` (background-only, base64 data URI).

Supported input formats: `png`, `jpg`/`jpeg`, `webp`, `gif`. Unknown extensions are rejected before upload.

### Other available endpoints

| Endpoint | Credits | Description |
|---|---|---|
| `POST /api/ocr` | 1 | Extract text with bounding boxes and language detection |
| `POST /api/inpaint` | 0.02 | Remove text from image using a mask |

## Architecture

All logic lives in `src/main.rs` (single-file).

### Dependencies (`Cargo.toml`)

| Crate | Version | Purpose |
|---|---|---|
| `reqwest` | 0.13 | Async HTTP client — multipart form upload + JSON response |
| `tokio` | 1 (full) | Async runtime |
| `dialoguer` | 0.12 | Interactive prompts — `Select`, `Input`, `Password` |
| `indicatif` | 0.18 | Animated spinner with `{elapsed_precise}` ms counter per file |
| `console` | 0.16 | Colorful terminal output (magenta, cyan, green, yellow, red) |
| `serde` / `serde_json` | 1 | Deserialize API JSON response |
| `base64` | 0.22 | Decode base64 image data from response |
| `dirs` | 6 | Resolve `~` to home directory |
| `anyhow` | 1 | Ergonomic error handling throughout |

### Key implementation details

- **Single `reqwest::Client`** created in `run()` with a 120s timeout, passed into each `translate_file()` call — not recreated per file.
- **`translate_file()` returns `Result<String>`** — the output filename (with response-derived extension) so the caller can display the exact saved path.
- **Config file permissions** — written with `OpenOptions::mode(0o600)` and then `set_permissions(0o600)` on Unix to ensure owner-only access even on existing files.
- **Path safety** — `Path::file_name()` is used to extract the basename before building the multipart `file_name` field and the output path, preventing path traversal.
- **Error propagation** — `run() -> Result<()>` uses `?` throughout; `main()` matches on `run().await` and calls `std::process::exit(1)` on error, printing a styled error message.
- **Sequential batch processing** — files are translated one at a time in order; each gets a spinner with `{elapsed_precise}` that clears when done.
- **Graceful Ctrl+C** — an `Arc<AtomicBool>` stop flag is set by a `tokio::signal::ctrl_c()` listener; checked after each file completes.

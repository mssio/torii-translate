# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build          # build the project
cargo run            # run the CLI
cargo check          # fast type/compile check without linking
cargo clippy         # lint
```

## Project Goal

A Rust CLI tool that sends manga/comic images to the Torii Translate API and saves the translated results locally. The app is interactive — it prompts the user through configuration and file selection before making any API calls.

## Intended Behavior (from instruction.txt)

1. **API key** — prompt once, persist to `~/.mss-torii-translate/config`. Skip prompt if the key already exists.
2. **Translator model** — interactive selection menu:
   - Gemini 2.5 Flash Lite (1 credit) → `gemini-2.5-flash`
   - Deepseek (1 credit) → `deepseek`
   - Grok 4.1 Fast (1 credit) → `grok-4-fast`
   - Kimi K2.5 (2 credits) → `kimi-k2`
   - GPT 5.1 (2+ credits) → `gpt-5`
   - Gemini 3 Flash (2+ credits) → `gemini-3-flash`
3. **Single vs. batch** — user chooses one image or a range (e.g. `001.png`–`003.png` calls the API three times).
4. **Single image** — prompts for filename; display the current directory name in the prompt.
5. **Batch images** — prompts for start filename and end filename; derives the file list by incrementing the numeric stem.
6. **Output** — decoded base64 `image` field written to `./translated-result/<original-filename>`.

## API

`POST https://api.toriitranslate.com/api/v2/upload` — multipart form upload.

Key form fields: `target_lang`, `translator`, `font`, `text_align`, `stroke_disabled`, `min_font_size`, `file`.

Response JSON contains `image` (translated, base64 PNG) and `inpainted` (background-only, base64 PNG).

## Architecture Notes

The project is early-stage — `src/main.rs` is a stub and `clap` is the only dependency. The full implementation will need:

- An HTTP client (e.g. `reqwest` with `multipart` + `tokio`) for async API calls.
- An interactive prompt library (e.g. `dialoguer`) for the menu/input flows.
- Config file read/write for the API key (`~/.mss-torii-translate/config`).
- Base64 decoding to write the response image bytes to disk.

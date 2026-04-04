use anyhow::{Context, Result, bail};
use base64::Engine;
use console::style;
use dialoguer::{Input, Password, Select, theme::ColorfulTheme};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

// --- types ---

struct Config {
    api_key: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    image: String,
}

struct TranslatorOption {
    label: &'static str,
    value: &'static str,
}

enum Mode {
    Single,
    Batch,
}

const TRANSLATORS: &[TranslatorOption] = &[
    TranslatorOption { label: "Gemini 2.5 Flash Lite  (1 credit)", value: "gemini-2.5-flash" },
    TranslatorOption { label: "Deepseek               (1 credit)", value: "deepseek" },
    TranslatorOption { label: "Grok 4.1 Fast          (1 credit)", value: "grok-4-fast" },
    TranslatorOption { label: "Kimi K2.5              (2 credits)", value: "kimi-k2" },
    TranslatorOption { label: "GPT 5.1                (2+ credits)", value: "gpt-5" },
    TranslatorOption { label: "Gemini 3 Flash         (2+ credits)", value: "gemini-3-flash" },
];

// --- banner ---

fn print_banner() {
    println!();
    println!(
        "  {}  {}",
        style("⛩").bold(),
        style("Torii Translate").bold().magenta()
    );
    println!(
        "  {}",
        style("Manga image translation powered by AI").dim()
    );
    println!("  {}", style("─".repeat(42)).dim());
    println!();
}

fn section(label: &str) {
    println!();
    println!("  {}", style(label).bold().cyan());
}

fn success(msg: &str) {
    println!("  {} {}", style("✓").bold().green(), style(msg).green());
}

fn info(msg: &str) {
    println!("  {} {}", style("›").dim(), style(msg).dim());
}

fn progress(msg: &str) {
    println!("  {} {}", style("◆").bold().yellow(), style(msg).bold());
}

fn saved(filename: &str) {
    println!(
        "  {} {} {}",
        style("✓").bold().green(),
        style("Saved →").green(),
        style(format!("translated-result/{}", filename)).bold().green()
    );
}

fn err(msg: &str) {
    println!("  {} {}", style("✗").bold().red(), style(msg).red());
}

// --- config ---

fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".mss-torii-translate").join("config"))
}

fn load_config() -> Option<Config> {
    let path = config_path().ok()?;
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        if let Some(("api_key", val)) = line.split_once('=') {
            let key = val.trim().to_string();
            if !key.is_empty() {
                return Some(Config { api_key: key });
            }
        }
    }
    None
}

fn save_config(api_key: &str) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create config directory")?;
    }
    fs::write(&path, format!("api_key={}\n", api_key)).context("failed to write config")?;
    Ok(())
}

fn ensure_api_key() -> Result<String> {
    if let Some(cfg) = load_config() {
        info("API key loaded from ~/.mss-torii-translate/config");
        return Ok(cfg.api_key);
    }

    section("API Key");
    info("No API key found. Enter your Torii Translate key to continue.");
    println!();

    let theme = ColorfulTheme::default();
    let key: String = Password::with_theme(&theme)
        .with_prompt(format!("{}", style("API key").bold()))
        .interact()
        .context("failed to read API key")?;
    let key = key.trim().to_string();
    if key.is_empty() {
        bail!("API key cannot be empty");
    }
    save_config(&key)?;
    success("API key saved to ~/.mss-torii-translate/config");
    Ok(key)
}

// --- prompts ---

fn prompt_translator() -> Result<&'static str> {
    section("Translator");
    let labels: Vec<&str> = TRANSLATORS.iter().map(|t| t.label).collect();
    let theme = ColorfulTheme::default();
    let idx = Select::with_theme(&theme)
        .with_prompt(format!("{}", style("Model").bold()))
        .items(&labels)
        .default(0)
        .interact()
        .context("failed to read translator selection")?;
    Ok(TRANSLATORS[idx].value)
}

fn prompt_mode() -> Result<Mode> {
    section("Input");
    let items = [
        format!("{}  Single image", style("❯").magenta()),
        format!("{}  Batch  (range of images)", style("❯").magenta()),
    ];
    let theme = ColorfulTheme::default();
    let idx = Select::with_theme(&theme)
        .with_prompt(format!("{}", style("Mode").bold()))
        .items(&items)
        .default(0)
        .interact()
        .context("failed to read mode selection")?;
    Ok(if idx == 0 { Mode::Single } else { Mode::Batch })
}

fn prompt_single_file() -> Result<String> {
    let cwd = std::env::current_dir().context("failed to get current directory")?;
    let dir_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("current directory");
    let theme = ColorfulTheme::default();
    let filename: String = Input::with_theme(&theme)
        .with_prompt(format!(
            "{} {}",
            style("Filename in").bold(),
            style(format!("[{}]", dir_name)).bold().cyan()
        ))
        .interact_text()
        .context("failed to read filename")?;
    Ok(filename.trim().to_string())
}

fn prompt_batch_range() -> Result<(String, String)> {
    let cwd = std::env::current_dir().context("failed to get current directory")?;
    let dir_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("current directory");
    let theme = ColorfulTheme::default();
    let start: String = Input::with_theme(&theme)
        .with_prompt(format!(
            "{} {}",
            style("Start filename in").bold(),
            style(format!("[{}]", dir_name)).bold().cyan()
        ))
        .interact_text()
        .context("failed to read start filename")?;
    let end: String = Input::with_theme(&theme)
        .with_prompt(format!("{}", style("End filename").bold()))
        .interact_text()
        .context("failed to read end filename")?;
    Ok((start.trim().to_string(), end.trim().to_string()))
}

// --- batch ---

fn split_stem_ext(filename: &str) -> Result<(&str, &str)> {
    let dot = filename
        .rfind('.')
        .with_context(|| format!("filename '{}' has no extension", filename))?;
    Ok((&filename[..dot], &filename[dot..]))
}

fn generate_batch_filenames(start: &str, end: &str) -> Result<Vec<String>> {
    let (start_stem, ext) = split_stem_ext(start)?;
    let (end_stem, _) = split_stem_ext(end)?;
    let width = start_stem.len();
    let start_n: u64 = start_stem
        .parse()
        .with_context(|| format!("start filename stem '{}' is not a number", start_stem))?;
    let end_n: u64 = end_stem
        .parse()
        .with_context(|| format!("end filename stem '{}' is not a number", end_stem))?;
    if start_n > end_n {
        bail!("start filename must come before end filename");
    }
    let filenames = (start_n..=end_n)
        .map(|n| format!("{:0>width$}{}", n, ext, width = width))
        .collect();
    Ok(filenames)
}

// --- api ---

fn mime_for_ext(filename: &str) -> &'static str {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    }
}

async fn translate_file(api_key: &str, translator: &str, filename: &str) -> Result<()> {
    let bytes = tokio::fs::read(filename)
        .await
        .with_context(|| format!("failed to read file '{}'", filename))?;

    let mime = mime_for_ext(filename);
    let file_part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str(mime)
        .context("invalid MIME type")?;

    let form = reqwest::multipart::Form::new()
        .text("target_lang", "en")
        .text("translator", translator.to_string())
        .text("font", "wildwords")
        .text("text_align", "auto")
        .text("stroke_disabled", "false")
        .text("min_font_size", "12")
        .part("file", file_part);

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.toriitranslate.com/api/v2/upload")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .context("failed to send request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("API error {}: {}", status, body);
    }

    let api_response: ApiResponse = response.json().await.context("failed to parse API response")?;

    let b64_data = api_response
        .image
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(&api_response.image);

    let image_bytes = base64::engine::general_purpose::STANDARD
        .decode(b64_data)
        .context("failed to decode base64 image")?;

    let out_path = format!("./translated-result/{}", filename);
    tokio::fs::write(&out_path, &image_bytes)
        .await
        .with_context(|| format!("failed to write output file '{}'", out_path))?;

    Ok(())
}

// --- main ---

#[tokio::main]
async fn main() -> Result<()> {
    print_banner();

    let api_key = ensure_api_key()?;
    let translator = prompt_translator()?;
    let mode = prompt_mode()?;

    let filenames: Vec<String> = match mode {
        Mode::Single => {
            let f = prompt_single_file()?;
            vec![f]
        }
        Mode::Batch => {
            let (start, end) = prompt_batch_range()?;
            match generate_batch_filenames(&start, &end) {
                Ok(files) => files,
                Err(e) => {
                    err(&format!("{}", e));
                    std::process::exit(1);
                }
            }
        }
    };

    tokio::fs::create_dir_all("./translated-result")
        .await
        .context("failed to create output directory")?;

    section("Translating");
    println!(
        "  {} {} file{}",
        style("Processing").bold(),
        style(filenames.len()).bold().magenta(),
        if filenames.len() == 1 { "" } else { "s" }
    );
    println!();

    let mut ok_count = 0usize;
    let mut fail_count = 0usize;

    for filename in &filenames {
        progress(&format!("{}", style(filename).bold()));
        match translate_file(&api_key, translator, filename).await {
            Ok(()) => {
                saved(filename);
                ok_count += 1;
            }
            Err(e) => {
                err(&format!("{}: {}", filename, e));
                fail_count += 1;
            }
        }
    }

    println!();
    println!("  {}", style("─".repeat(42)).dim());
    if fail_count == 0 {
        println!(
            "  {} {} file{} translated successfully",
            style("✓").bold().green(),
            style(ok_count).bold().green(),
            if ok_count == 1 { "" } else { "s" }
        );
    } else {
        println!(
            "  {} {}/{} succeeded  {} failed",
            style("◆").yellow(),
            style(ok_count).bold().green(),
            filenames.len(),
            style(fail_count).bold().red()
        );
    }
    println!();

    Ok(())
}

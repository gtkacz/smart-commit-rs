use anyhow::{bail, Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::AppConfig;
use crate::interpolation::interpolate;

const TOTAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_RESPONSE_BYTES: usize = 1_048_576;
const MAX_ERROR_BODY_BYTES: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Interactive,
    Quiet,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RequestFormat {
    Gemini,
    OpenAiCompat,
    Anthropic,
    LmStudio,
}

struct ProviderDef {
    api_url: &'static str,
    api_headers: &'static str,
    default_model: &'static str,
    format: RequestFormat,
    response_path: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Harness {
    ClaudeCode,
    Codex,
    OpenCode,
    GeminiCli,
}

impl Harness {
    fn executable(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::GeminiCli => "gemini",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::OpenCode => "OpenCode",
            Self::GeminiCli => "Gemini CLI",
        }
    }
}

fn get_harness(name: &str) -> Option<Harness> {
    match name {
        "claude_code" | "claude-code" => Some(Harness::ClaudeCode),
        "codex" => Some(Harness::Codex),
        "opencode" => Some(Harness::OpenCode),
        "gemini_cli" | "gemini-cli" => Some(Harness::GeminiCli),
        _ => None,
    }
}

pub fn is_local_harness(provider: &str) -> bool {
    get_harness(provider).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelListParser {
    Data,
    Gemini,
    Models,
    Ollama,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelListSpec {
    pub url: String,
    pub headers: String,
    pub parser: ModelListParser,
}

/// Built-in provider definitions
fn get_provider(name: &str) -> Option<ProviderDef> {
    match name {
        "gemini" => Some(ProviderDef {
            api_url: "https://generativelanguage.googleapis.com/v1beta/models/$ACR_MODEL:generateContent?key=$ACR_API_KEY",
            api_headers: "",
            default_model: "gemini-2.0-flash",
            format: RequestFormat::Gemini,
            response_path: "candidates.0.content.parts.0.text",
        }),
        "openai" => Some(ProviderDef {
            api_url: "https://api.openai.com/v1/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "gpt-4o-mini",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "anthropic" => Some(ProviderDef {
            api_url: "https://api.anthropic.com/v1/messages",
            api_headers: "x-api-key: $ACR_API_KEY, anthropic-version: 2023-06-01",
            default_model: "claude-sonnet-4-20250514",
            format: RequestFormat::Anthropic,
            response_path: "content.0.text",
        }),
        "groq" => Some(ProviderDef {
            api_url: "https://api.groq.com/openai/v1/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "llama-3.3-70b-versatile",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "grok" => Some(ProviderDef {
            api_url: "https://api.x.ai/v1/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "grok-3",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "deepseek" => Some(ProviderDef {
            api_url: "https://api.deepseek.com/v1/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "deepseek-chat",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "openrouter" => Some(ProviderDef {
            api_url: "https://openrouter.ai/api/v1/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "openai/gpt-4o-mini",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "mistral" => Some(ProviderDef {
            api_url: "https://api.mistral.ai/v1/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "mistral-small-latest",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "together" => Some(ProviderDef {
            api_url: "https://api.together.xyz/v1/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "fireworks" => Some(ProviderDef {
            api_url: "https://api.fireworks.ai/inference/v1/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "accounts/fireworks/models/llama-v3p3-70b-instruct",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "perplexity" => Some(ProviderDef {
            api_url: "https://api.perplexity.ai/chat/completions",
            api_headers: "Authorization: Bearer $ACR_API_KEY",
            default_model: "sonar",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        "lm_studio" => Some(ProviderDef {
            api_url: "http://localhost:1234/api/v1/chat",
            api_headers: "Content-Type: application/json",
            default_model: "qwen/qwen3.5-35b-a3b",
            format: RequestFormat::LmStudio,
            response_path: "output",
        }),
        "ollama" => Some(ProviderDef {
            api_url: "http://localhost:11434/v1/chat/completions",
            api_headers: "Content-Type: application/json",
            default_model: "llama3",
            format: RequestFormat::OpenAiCompat,
            response_path: "choices.0.message.content",
        }),
        _ => None,
    }
}

/// Get the default model for a built-in provider, or empty string for unknown providers.
pub fn default_model_for(provider: &str) -> &'static str {
    get_provider(provider).map_or("", |p| p.default_model)
}

/// Resolve model-discovery metadata for a provider. An explicit endpoint is
/// used only when it has a recognizable chat-completions shape, preventing
/// guesses against arbitrary custom APIs.
pub fn model_list_spec(cfg: &AppConfig) -> Option<ModelListSpec> {
    if is_local_harness(&cfg.provider) {
        return None;
    }
    if !cfg.api_url.trim().is_empty() {
        let url = derive_models_url(&cfg.api_url)?;
        let headers = if cfg.api_headers.is_empty() {
            get_provider(&cfg.provider)
                .map(|provider| provider.api_headers)
                .unwrap_or("")
                .to_string()
        } else {
            cfg.api_headers.clone()
        };
        return Some(ModelListSpec {
            url,
            headers,
            parser: ModelListParser::Data,
        });
    }

    let (url, parser) = match cfg.provider.as_str() {
        "openai" => ("https://api.openai.com/v1/models", ModelListParser::Data),
        "anthropic" => ("https://api.anthropic.com/v1/models", ModelListParser::Data),
        "gemini" => (
            "https://generativelanguage.googleapis.com/v1beta/models?key=$ACR_API_KEY",
            ModelListParser::Gemini,
        ),
        "groq" => (
            "https://api.groq.com/openai/v1/models",
            ModelListParser::Data,
        ),
        "grok" => ("https://api.x.ai/v1/models", ModelListParser::Data),
        "deepseek" => ("https://api.deepseek.com/v1/models", ModelListParser::Data),
        "openrouter" => ("https://openrouter.ai/api/v1/models", ModelListParser::Data),
        "mistral" => ("https://api.mistral.ai/v1/models", ModelListParser::Data),
        "together" => ("https://api.together.xyz/v1/models", ModelListParser::Data),
        "fireworks" => (
            "https://api.fireworks.ai/inference/v1/models",
            ModelListParser::Data,
        ),
        "lm_studio" => (
            "http://localhost:1234/api/v1/models",
            ModelListParser::Models,
        ),
        "ollama" => ("http://localhost:11434/api/tags", ModelListParser::Ollama),
        // Perplexity's current list endpoint describes a different API surface.
        "perplexity" => return None,
        _ => return None,
    };
    let headers = if cfg.api_headers.is_empty() {
        get_provider(&cfg.provider)?.api_headers.to_string()
    } else {
        cfg.api_headers.clone()
    };
    Some(ModelListSpec {
        url: url.to_string(),
        headers,
        parser,
    })
}

fn derive_models_url(api_url: &str) -> Option<String> {
    let trimmed = api_url.trim_end_matches('/');
    let suffix = "/chat/completions";
    trimmed
        .strip_suffix(suffix)
        .map(|base| format!("{base}/models"))
}

/// Returns true if the provider requires an API key (i.e. its header template references $ACR_API_KEY).
pub fn provider_requires_api_key(cfg: &AppConfig) -> bool {
    if is_local_harness(&cfg.provider) {
        return false;
    }
    let headers = if let Some(def) = get_provider(&cfg.provider) {
        if cfg.api_headers.is_empty() {
            def.api_headers.to_string()
        } else {
            cfg.api_headers.clone()
        }
    } else {
        cfg.api_headers.clone()
    };

    let url = if let Some(def) = get_provider(&cfg.provider) {
        if cfg.api_url.is_empty() {
            def.api_url.to_string()
        } else {
            cfg.api_url.clone()
        }
    } else {
        cfg.api_url.clone()
    };

    headers.contains("$ACR_API_KEY") || url.contains("$ACR_API_KEY")
}

#[derive(Debug)]
pub enum LlmCallError {
    HttpError { code: u16, body: String },
    TransportError(String),
    Other(anyhow::Error),
}

impl LlmCallError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::TransportError(_) => true,
            Self::HttpError { code, .. } => {
                // A provider can reject a request as too large for its current
                // model or quota while another configured provider accepts it.
                matches!(*code, 408 | 409 | 413 | 425 | 429) || (500..=599).contains(code)
            }
            Self::Other(_) => false,
        }
    }
}

impl std::fmt::Display for LlmCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmCallError::HttpError { code, body } => {
                write!(f, "API returned HTTP {code}: {body}")
            }
            LlmCallError::TransportError(msg) => write!(f, "Network error: {msg}"),
            LlmCallError::Other(e) => write!(f, "{e}"),
        }
    }
}

fn call_llm_inner(
    cfg: &AppConfig,
    system_prompt: &str,
    user_prompt: &str,
    deadline: Instant,
    output_mode: OutputMode,
) -> Result<String, LlmCallError> {
    if let Some(harness) = get_harness(&cfg.provider) {
        return call_harness(
            harness,
            cfg,
            system_prompt,
            user_prompt,
            deadline,
            output_mode,
        );
    }

    let (url, headers_raw, format, response_path) =
        resolve_provider(cfg).map_err(LlmCallError::Other)?;

    let url = interpolate(&url, cfg).map_err(LlmCallError::Other)?;
    let headers_raw = interpolate(&headers_raw, cfg).map_err(LlmCallError::Other)?;

    let body = build_request_body(
        format,
        &cfg.model,
        system_prompt,
        user_prompt,
        cfg.max_output_tokens,
    );
    let body = serde_json::to_vec(&body).map_err(|error| {
        LlmCallError::Other(anyhow::anyhow!("Failed to serialize API request: {error}"))
    })?;
    let headers = parse_headers(&headers_raw).map_err(LlmCallError::Other)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(LlmCallError::TransportError(
            "total request deadline exceeded".into(),
        ));
    }

    let spinner = (output_mode == OutputMode::Interactive).then(|| {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg} {elapsed}")
                .unwrap(),
        );
        spinner.set_message("Generating commit message...");
        spinner.enable_steady_tick(Duration::from_millis(80));
        spinner
    });

    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    let agent = AGENT.get_or_init(ureq::Agent::new_with_defaults);
    let mut req = agent.post(&url);
    for (key, val) in &headers {
        req = req.header(key, val);
    }
    if !headers
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case("content-type"))
    {
        req = req.header("Content-Type", "application/json");
    }

    let response = req
        .config()
        .timeout_global(Some(remaining))
        .http_status_as_error(false)
        .build()
        .send(body.as_slice());

    if let Some(spinner) = spinner {
        spinner.finish_and_clear();
    }

    let response = match response {
        Ok(resp) => resp,
        Err(error) => {
            return Err(LlmCallError::TransportError(redact(
                &error.to_string(),
                &cfg.api_key,
            )));
        }
    };
    let status = response.status().as_u16();
    if status >= 400 {
        let body = read_bounded(response.into_body().into_reader(), MAX_ERROR_BODY_BYTES)
            .unwrap_or_else(|_| "<response body unavailable>".into());
        let body = redact(&body, &cfg.api_key);
        return Err(LlmCallError::HttpError { code: status, body });
    }

    let response_body = read_bounded(response.into_body().into_reader(), MAX_RESPONSE_BYTES)
        .map_err(LlmCallError::Other)?;
    let json: Value = serde_json::from_str(&response_body).map_err(|e| {
        LlmCallError::Other(anyhow::anyhow!("Failed to parse API response as JSON: {e}"))
    })?;

    let message = extract_message(&json, format, &response_path).map_err(|e| {
        LlmCallError::Other(anyhow::anyhow!(
            "Failed to extract message from response at path '{}'. Response: {}\nError: {}",
            response_path,
            bounded_json_diagnostic(&json, &cfg.api_key),
            e
        ))
    })?;

    if message.trim().is_empty() {
        return Err(LlmCallError::Other(anyhow::anyhow!(
            "LLM returned an empty commit message. Response: {}",
            bounded_json_diagnostic(&json, &cfg.api_key)
        )));
    }

    Ok(message)
}

/// Call LLM with fallback support. Returns (message, fallback_preset_name_if_used).
pub fn call_llm_with_fallback(
    cfg: &AppConfig,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<(String, Option<String>)> {
    call_llm_with_fallback_mode(cfg, system_prompt, user_prompt, OutputMode::Interactive)
}

pub fn call_llm_with_fallback_mode(
    cfg: &AppConfig,
    system_prompt: &str,
    user_prompt: &str,
    output_mode: OutputMode,
) -> Result<(String, Option<String>)> {
    let deadline = Instant::now() + TOTAL_REQUEST_TIMEOUT;
    match call_llm_inner(cfg, system_prompt, user_prompt, deadline, output_mode) {
        Ok(msg) => Ok((msg, None)),
        Err(primary_error) => {
            if !cfg.fallback_enabled || !primary_error.is_retryable() {
                anyhow::bail!("{primary_error}");
            }
            let presets_file = match crate::preset::load_presets() {
                Ok(f) => f,
                Err(_) => anyhow::bail!("{primary_error}"),
            };

            if presets_file.fallback.order.is_empty() {
                anyhow::bail!("{primary_error}");
            }

            let current_fields = crate::preset::fields_from_config(cfg);
            let mut errors = vec![format!("Primary ({primary_error})")];

            for &preset_id in &presets_file.fallback.order {
                let preset = match presets_file.presets.iter().find(|p| p.id == preset_id) {
                    Some(p) => p,
                    None => continue,
                };

                // Skip only presets identical to the active config; one that
                // differs in any field (headers included) is a real alternative
                if preset.fields == current_fields {
                    continue;
                }

                if output_mode == OutputMode::Interactive {
                    eprintln!(
                        "{} Primary failed, trying: {}...",
                        "fallback:".yellow().bold(),
                        preset.name
                    );
                }

                let mut temp_cfg = cfg.clone();
                crate::preset::apply_preset_to_config(&mut temp_cfg, preset);

                match call_llm_inner(&temp_cfg, system_prompt, user_prompt, deadline, output_mode) {
                    Ok(msg) => return Ok((msg, Some(preset.name.clone()))),
                    Err(error) => {
                        let retryable = error.is_retryable();
                        errors.push(format!("{} ({error})", preset.name));
                        if retryable {
                            continue;
                        }
                        anyhow::bail!("LLM fallback stopped: {}", errors.join(", "));
                    }
                }
            }

            anyhow::bail!("All LLM providers failed: {}", errors.join(", "));
        }
    }
}

/// Call the LLM API and return the generated commit message
pub fn call_llm(cfg: &AppConfig, system_prompt: &str, user_prompt: &str) -> Result<String> {
    let (msg, _) = call_llm_with_fallback(cfg, system_prompt, user_prompt)?;
    Ok(msg)
}

/// Generate a commit message for the staged diff and validate it, feeding the
/// validator error back to the model for one corrective retry before giving up.
/// Returns (message, fallback_preset_name_if_used).
pub fn generate_validated_message(
    cfg: &AppConfig,
    system_prompt: &str,
    diff: &str,
) -> Result<(String, Option<String>)> {
    generate_validated_message_with_context(cfg, system_prompt, diff, &[], OutputMode::Interactive)
}

pub fn generate_validated_message_with_context(
    cfg: &AppConfig,
    system_prompt: &str,
    diff: &str,
    previous: &[String],
    output_mode: OutputMode,
) -> Result<(String, Option<String>)> {
    let user_prompt = crate::prompt::build_user_prompt_avoiding(diff, previous);
    let (raw, fallback) =
        call_llm_with_fallback_mode(cfg, system_prompt, &user_prompt, output_mode)
            .context("LLM API call failed")?;
    let message = crate::prompt::clean_commit_message(&raw);
    let error = match crate::prompt::validate_commit_message(&message, cfg) {
        Ok(()) => return Ok((message, fallback)),
        Err(error) => error,
    };

    if output_mode == OutputMode::Interactive {
        println!(
            "  {} LLM message failed validation ({error}); requesting a correction",
            "note:".yellow().bold()
        );
    }
    let mut correction_prompt =
        crate::prompt::build_correction_prompt(diff, &message, &error.to_string());
    if !previous.is_empty() {
        let recent = previous
            .iter()
            .enumerate()
            .map(|(index, candidate)| {
                format!(
                    "<candidate_{}>\n{}\n</candidate_{}>",
                    index + 1,
                    candidate,
                    index + 1
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        correction_prompt.push_str(&format!(
            "\n\n<recent_candidates>\n{recent}\n</recent_candidates>\n\nThe corrected result must remain meaningfully distinct from these recent candidates."
        ));
    }
    let (raw, fallback) =
        call_llm_with_fallback_mode(cfg, system_prompt, &correction_prompt, output_mode)
            .context("LLM API call failed")?;
    let message = crate::prompt::clean_commit_message(&raw);
    crate::prompt::validate_commit_message(&message, cfg)
        .context("LLM returned an invalid commit message even after a corrective retry")?;
    Ok((message, fallback))
}

fn resolve_provider(cfg: &AppConfig) -> Result<(String, String, RequestFormat, String)> {
    if let Some(def) = get_provider(&cfg.provider) {
        let url = if cfg.api_url.is_empty() {
            def.api_url.to_string()
        } else {
            cfg.api_url.clone()
        };
        let headers = if cfg.api_headers.is_empty() {
            def.api_headers.to_string()
        } else {
            cfg.api_headers.clone()
        };
        Ok((url, headers, def.format, def.response_path.to_string()))
    } else {
        // Custom provider: require API URL, default to OpenAI-compatible format
        if cfg.api_url.is_empty() {
            bail!(
                "Unknown provider '{}'. Set {} for custom providers.",
                cfg.provider.yellow(),
                "ACR_API_URL".yellow()
            );
        }
        Ok((
            cfg.api_url.clone(),
            cfg.api_headers.clone(),
            RequestFormat::OpenAiCompat,
            "choices.0.message.content".to_string(),
        ))
    }
}

struct HarnessOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn call_harness(
    harness: Harness,
    cfg: &AppConfig,
    system_prompt: &str,
    user_prompt: &str,
    deadline: Instant,
    output_mode: OutputMode,
) -> Result<String, LlmCallError> {
    let mut last_message = None;
    let mut command =
        harness_command(harness, cfg, &mut last_message).map_err(LlmCallError::Other)?;
    let prompt = combined_harness_prompt(system_prompt, user_prompt);

    let spinner = (output_mode == OutputMode::Interactive).then(new_spinner);
    let output = run_harness_process(&mut command, &prompt, deadline, harness);
    if let Some(spinner) = spinner {
        spinner.finish_and_clear();
    }
    let output = output?;

    if !output.status.success() {
        let diagnostic = if output.stderr.trim().is_empty() {
            output.stdout.trim()
        } else {
            output.stderr.trim()
        };
        let diagnostic = if diagnostic.is_empty() {
            "no diagnostic output".to_string()
        } else {
            redact(diagnostic, &cfg.api_key)
        };
        return Err(LlmCallError::Other(anyhow::anyhow!(
            "{} exited with {}: {}",
            harness.display_name(),
            output.status,
            diagnostic
        )));
    }

    let message = match harness {
        Harness::ClaudeCode => extract_json_field(&output.stdout, "result", harness)?,
        Harness::Codex => {
            let mut file = last_message.expect("Codex always creates a last-message file");
            file.as_file_mut()
                .seek(SeekFrom::Start(0))
                .map_err(|error| harness_io_error(harness, "read output", error))?;
            read_bounded(file.as_file_mut(), MAX_RESPONSE_BYTES).map_err(|error| {
                LlmCallError::Other(error.context("Failed to read Codex output"))
            })?
        }
        Harness::OpenCode => extract_opencode_text(&output.stdout)?,
        Harness::GeminiCli => extract_json_field(&output.stdout, "response", harness)?,
    };

    if message.trim().is_empty() {
        return Err(LlmCallError::Other(anyhow::anyhow!(
            "{} returned an empty commit message",
            harness.display_name()
        )));
    }
    Ok(message)
}

fn harness_command(
    harness: Harness,
    cfg: &AppConfig,
    last_message: &mut Option<tempfile::NamedTempFile>,
) -> Result<Command> {
    let mut command = Command::new(harness.executable());
    match harness {
        Harness::ClaudeCode => {
            command.args([
                "--print",
                "--output-format",
                "json",
                "--no-session-persistence",
                "--permission-mode",
                "plan",
                "--tools",
                "",
            ]);
        }
        Harness::Codex => {
            let output = tempfile::NamedTempFile::new()
                .context("Failed to create temporary Codex output file")?;
            command.args([
                "exec",
                "--sandbox",
                "read-only",
                "--ephemeral",
                "--color",
                "never",
                "--output-last-message",
            ]);
            command.arg(output.path());
            *last_message = Some(output);
        }
        Harness::OpenCode => {
            command.args(["run", "--format", "json", "--agent", "plan"]);
        }
        Harness::GeminiCli => {
            command.args([
                "--prompt",
                "",
                "--output-format",
                "json",
                "--approval-mode",
                "plan",
            ]);
        }
    }
    if !cfg.model.trim().is_empty() {
        command.arg("--model").arg(cfg.model.trim());
    }
    if harness == Harness::Codex {
        command.arg("-");
    }
    Ok(command)
}

fn combined_harness_prompt(system_prompt: &str, user_prompt: &str) -> String {
    format!(
        "<system_instructions>\n{system_prompt}\n</system_instructions>\n\n{user_prompt}\n\nDo not inspect the workspace or call tools. Return only the requested commit message."
    )
}

fn new_spinner() -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg} {elapsed}")
            .unwrap(),
    );
    spinner.set_message("Generating commit message...");
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

fn run_harness_process(
    command: &mut Command,
    prompt: &str,
    deadline: Instant,
    harness: Harness,
) -> Result<HarnessOutput, LlmCallError> {
    let mut stdin = tempfile::tempfile()
        .map_err(|error| harness_io_error(harness, "create stdin buffer", error))?;
    stdin
        .write_all(prompt.as_bytes())
        .map_err(|error| harness_io_error(harness, "write stdin buffer", error))?;
    stdin
        .seek(SeekFrom::Start(0))
        .map_err(|error| harness_io_error(harness, "rewind stdin buffer", error))?;
    let mut stdout = tempfile::tempfile()
        .map_err(|error| harness_io_error(harness, "create stdout buffer", error))?;
    let mut stderr = tempfile::tempfile()
        .map_err(|error| harness_io_error(harness, "create stderr buffer", error))?;
    command
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout.try_clone().map_err(|error| {
            harness_io_error(harness, "create stdout buffer", error)
        })?))
        .stderr(Stdio::from(stderr.try_clone().map_err(|error| {
            harness_io_error(harness, "create stderr buffer", error)
        })?));

    let mut child = command.spawn().map_err(|error| {
        LlmCallError::Other(anyhow::anyhow!(
            "Failed to start {} executable '{}': {}. Install and authenticate the CLI first.",
            harness.display_name(),
            harness.executable(),
            error
        ))
    })?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(LlmCallError::TransportError(format!(
                    "{} exceeded the total request deadline",
                    harness.display_name()
                )));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(harness_io_error(harness, "wait for process", error));
            }
        }
    };

    stdout
        .seek(SeekFrom::Start(0))
        .map_err(|error| harness_io_error(harness, "read stdout", error))?;
    stderr
        .seek(SeekFrom::Start(0))
        .map_err(|error| harness_io_error(harness, "read stderr", error))?;
    let stdout = read_bounded(&mut stdout, MAX_RESPONSE_BYTES)
        .map_err(|error| LlmCallError::Other(error.context("Harness output was invalid")))?;
    let stderr = read_bounded(&mut stderr, MAX_ERROR_BODY_BYTES)
        .unwrap_or_else(|_| "<diagnostic output exceeded safety limit>".into());

    Ok(HarnessOutput {
        status,
        stdout,
        stderr,
    })
}

fn harness_io_error(harness: Harness, action: &str, error: std::io::Error) -> LlmCallError {
    LlmCallError::Other(anyhow::anyhow!(
        "Failed to {action} for {}: {error}",
        harness.display_name()
    ))
}

fn extract_json_field(output: &str, field: &str, harness: Harness) -> Result<String, LlmCallError> {
    let value: Value = serde_json::from_str(output).map_err(|error| {
        LlmCallError::Other(anyhow::anyhow!(
            "Failed to parse {} output as JSON: {error}",
            harness.display_name()
        ))
    })?;
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            LlmCallError::Other(anyhow::anyhow!(
                "{} JSON output did not contain a string '{}' field",
                harness.display_name(),
                field
            ))
        })
}

fn extract_opencode_text(output: &str) -> Result<String, LlmCallError> {
    let mut parts: Vec<(String, String)> = Vec::new();
    for (line_number, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(line).map_err(|error| {
            LlmCallError::Other(anyhow::anyhow!(
                "Failed to parse OpenCode JSON event on line {}: {}",
                line_number + 1,
                error
            ))
        })?;
        if event.get("type").and_then(Value::as_str) != Some("text") {
            continue;
        }
        let Some(part) = event.get("part") else {
            continue;
        };
        let Some(text) = part.get("text").and_then(Value::as_str) else {
            continue;
        };
        let id = part
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("line-{line_number}"));
        if let Some((_, existing)) = parts.iter_mut().find(|(existing_id, _)| *existing_id == id) {
            *existing = text.to_string();
        } else {
            parts.push((id, text.to_string()));
        }
    }
    Ok(parts.into_iter().map(|(_, text)| text).collect())
}

fn build_request_body(
    format: RequestFormat,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    max_output_tokens: usize,
) -> Value {
    match format {
        RequestFormat::Gemini => {
            serde_json::json!({
                "system_instruction": {
                    "parts": [{ "text": system_prompt }]
                },
                "contents": [{
                    "role": "user",
                    "parts": [{ "text": user_prompt }]
                }],
                "generationConfig": {
                    "temperature": 0,
                    "maxOutputTokens": max_output_tokens
                }
            })
        }
        RequestFormat::OpenAiCompat => {
            serde_json::json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_prompt }
                ],
                "max_tokens": max_output_tokens,
                "temperature": 0,
                "stream": false
            })
        }
        RequestFormat::Anthropic => {
            serde_json::json!({
                "model": model,
                "system": system_prompt,
                "messages": [
                    { "role": "user", "content": user_prompt }
                ],
                "max_tokens": max_output_tokens,
                "temperature": 0,
                "stream": false
            })
        }
        RequestFormat::LmStudio => {
            serde_json::json!({
                "model": model,
                "system_prompt": system_prompt,
                "input": user_prompt
            })
        }
    }
}

/// Parse "Key: Value, Key2: Value2" header string into pairs
pub(crate) fn parse_headers(raw: &str) -> Result<Vec<(String, String)>> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    if raw.trim_start().starts_with('{') {
        let value: Value = serde_json::from_str(raw).context("Invalid API headers JSON")?;
        let object = value
            .as_object()
            .context("API headers JSON must be an object")?;
        return object
            .iter()
            .map(|(key, value)| {
                let value = value
                    .as_str()
                    .context("API header values must be strings")?;
                validate_header(key, value)?;
                Ok((key.clone(), value.to_string()))
            })
            .collect();
    }
    raw.split(',')
        .map(|pair| {
            let (key, value) = pair
                .trim()
                .split_once(':')
                .context("Expected API header in 'Name: Value' format")?;
            let key = key.trim();
            let value = value.trim();
            validate_header(key, value)?;
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

fn validate_header(key: &str, value: &str) -> Result<()> {
    if key.is_empty()
        || value.is_empty()
        || key.chars().any(char::is_control)
        || value.chars().any(char::is_control)
    {
        bail!("API header names and values must be non-empty single-line text");
    }
    Ok(())
}

pub(crate) fn read_bounded(mut reader: impl Read, limit: usize) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .context("Failed to read API response")?;
    if bytes.len() > limit {
        bail!("API response exceeded the {limit}-byte safety limit");
    }
    String::from_utf8(bytes).context("API response was not valid UTF-8")
}

pub(crate) fn redact(value: &str, api_key: &str) -> String {
    if api_key.is_empty() {
        value.to_string()
    } else {
        value.replace(api_key, "[REDACTED]")
    }
}

fn bounded_json_diagnostic(value: &Value, api_key: &str) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| "<unavailable>".into());
    let redacted = redact(&serialized, api_key);
    if redacted.chars().count() <= MAX_ERROR_BODY_BYTES {
        redacted
    } else {
        format!(
            "{}…",
            redacted
                .chars()
                .take(MAX_ERROR_BODY_BYTES)
                .collect::<String>()
        )
    }
}

/// Walk a JSON value by a dot-separated path like "candidates.0.content.parts.0.text"
fn extract_by_path(value: &Value, path: &str) -> Result<String> {
    let mut current = value;
    for segment in path.split('.') {
        current = if let Ok(index) = segment.parse::<usize>() {
            current
                .get(index)
                .with_context(|| format!("Array index {index} out of bounds"))?
        } else {
            current
                .get(segment)
                .with_context(|| format!("Key '{segment}' not found"))?
        };
    }
    current
        .as_str()
        .map(|s| s.to_string())
        .with_context(|| "Expected string value at path end".to_string())
}

fn extract_message(value: &Value, format: RequestFormat, response_path: &str) -> Result<String> {
    match format {
        RequestFormat::LmStudio => {
            let output = value
                .get(response_path)
                .and_then(Value::as_array)
                .with_context(|| format!("Key '{response_path}' not found or is not an array"))?;

            let message = output
                .iter()
                .find(|item| item.get("type").and_then(Value::as_str) == Some("message"))
                .with_context(|| "No output item with type 'message' found".to_string())?;

            message
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
                .with_context(|| "Expected string 'content' in message output item".to_string())
        }
        _ => extract_by_path(value, response_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_headers_empty() {
        assert!(parse_headers("").unwrap().is_empty());
        assert!(parse_headers("   ").unwrap().is_empty());
    }

    #[test]
    fn test_parse_headers_single() {
        let headers = parse_headers("Authorization: Bearer abc123").unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Authorization");
        assert_eq!(headers[0].1, "Bearer abc123");
    }

    #[test]
    fn test_parse_headers_multiple() {
        let headers = parse_headers("X-Api-Key: key123, Content-Type: application/json").unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].0, "X-Api-Key");
        assert_eq!(headers[0].1, "key123");
        assert_eq!(headers[1].0, "Content-Type");
        assert_eq!(headers[1].1, "application/json");
    }

    #[test]
    fn test_parse_headers_trims_whitespace() {
        let headers = parse_headers("  Key  :  Value  ").unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Key");
        assert_eq!(headers[0].1, "Value");
    }

    #[test]
    fn test_parse_headers_rejects_invalid() {
        assert!(parse_headers("Valid: Header, InvalidNoColon, Another: One").is_err());
    }

    #[test]
    fn test_parse_headers_accepts_json_object() {
        let headers = parse_headers(r#"{"Authorization":"Bearer key","X-Mode":"safe"}"#).unwrap();
        assert!(headers.contains(&("Authorization".into(), "Bearer key".into())));
        assert!(headers.contains(&("X-Mode".into(), "safe".into())));
    }

    #[test]
    fn test_extract_by_path_simple() {
        let json = serde_json::json!({"message": "hello"});
        let result = extract_by_path(&json, "message").unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_extract_by_path_nested() {
        let json = serde_json::json!({"content": {"text": "nested"}});
        let result = extract_by_path(&json, "content.text").unwrap();
        assert_eq!(result, "nested");
    }

    #[test]
    fn test_extract_by_path_array_index() {
        let json = serde_json::json!({"items": ["first", "second"]});
        let result = extract_by_path(&json, "items.0").unwrap();
        assert_eq!(result, "first");
    }

    #[test]
    fn test_extract_by_path_complex() {
        let json = serde_json::json!({
            "choices": [{"message": {"content": "generated"}}]
        });
        let result = extract_by_path(&json, "choices.0.message.content").unwrap();
        assert_eq!(result, "generated");
    }

    #[test]
    fn test_extract_by_path_gemini_format() {
        let json = serde_json::json!({
            "candidates": [{"content": {"parts": [{"text": "gemini response"}]}}]
        });
        let result = extract_by_path(&json, "candidates.0.content.parts.0.text").unwrap();
        assert_eq!(result, "gemini response");
    }

    #[test]
    fn test_extract_by_path_anthropic_format() {
        let json = serde_json::json!({
            "content": [{"text": "anthropic response"}]
        });
        let result = extract_by_path(&json, "content.0.text").unwrap();
        assert_eq!(result, "anthropic response");
    }

    #[test]
    fn test_extract_by_path_key_not_found() {
        let json = serde_json::json!({"foo": "bar"});
        let result = extract_by_path(&json, "missing");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_extract_by_path_index_out_of_bounds() {
        let json = serde_json::json!({"items": ["only"]});
        let result = extract_by_path(&json, "items.5");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("out of bounds"));
    }

    #[test]
    fn test_extract_by_path_not_string() {
        let json = serde_json::json!({"number": 42});
        let result = extract_by_path(&json, "number");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Expected string"));
    }

    #[test]
    fn test_build_request_body_openai_compat() {
        let body = build_request_body(
            RequestFormat::OpenAiCompat,
            "gpt-4o",
            "system prompt",
            "user diff",
            640,
        );
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "system prompt");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "user diff");
        assert_eq!(body["max_tokens"], 640);
        assert_eq!(body["temperature"], 0);
    }

    #[test]
    fn test_build_request_body_gemini() {
        let body = build_request_body(
            RequestFormat::Gemini,
            "gemini-pro",
            "system prompt",
            "user diff",
            512,
        );
        assert_eq!(
            body["system_instruction"]["parts"][0]["text"],
            "system prompt"
        );
        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "user diff");
        assert_eq!(body["generationConfig"]["temperature"], 0);
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 512);
    }

    #[test]
    fn test_build_request_body_anthropic() {
        let body = build_request_body(
            RequestFormat::Anthropic,
            "claude-3-opus",
            "system prompt",
            "user diff",
            640,
        );
        assert_eq!(body["model"], "claude-3-opus");
        assert_eq!(body["system"], "system prompt");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "user diff");
        assert_eq!(body["max_tokens"], 640);
        assert_eq!(body["temperature"], 0);
    }

    #[test]
    fn test_build_request_body_lm_studio() {
        let body = build_request_body(
            RequestFormat::LmStudio,
            "qwen/qwen3.5-35b-a3b",
            "system prompt",
            "user diff",
            512,
        );
        assert_eq!(body["model"], "qwen/qwen3.5-35b-a3b");
        assert_eq!(body["system_prompt"], "system prompt");
        assert_eq!(body["input"], "user diff");
        assert!(body.get("messages").is_none());
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn test_get_provider_known() {
        assert!(get_provider("gemini").is_some());
        assert!(get_provider("openai").is_some());
        assert!(get_provider("anthropic").is_some());
        assert!(get_provider("groq").is_some());
        assert!(get_provider("grok").is_some());
        assert!(get_provider("deepseek").is_some());
        assert!(get_provider("openrouter").is_some());
        assert!(get_provider("mistral").is_some());
        assert!(get_provider("together").is_some());
        assert!(get_provider("fireworks").is_some());
        assert!(get_provider("perplexity").is_some());
        assert!(get_provider("lm_studio").is_some());
        assert!(get_provider("ollama").is_some());
    }

    #[test]
    fn test_get_provider_unknown() {
        assert!(get_provider("unknown").is_none());
        assert!(get_provider("custom").is_none());
    }

    #[test]
    fn test_get_provider_gemini_format() {
        let provider = get_provider("gemini").unwrap();
        assert_eq!(provider.format, RequestFormat::Gemini);
        assert!(provider
            .api_url
            .contains("generativelanguage.googleapis.com"));
        assert_eq!(provider.default_model, "gemini-2.0-flash");
    }

    #[test]
    fn test_get_provider_anthropic_format() {
        let provider = get_provider("anthropic").unwrap();
        assert_eq!(provider.format, RequestFormat::Anthropic);
        assert!(provider.api_url.contains("anthropic.com"));
        assert!(provider.api_headers.contains("anthropic-version"));
    }

    #[test]
    fn test_get_provider_openai_compat() {
        for name in &[
            "openai",
            "groq",
            "grok",
            "deepseek",
            "openrouter",
            "mistral",
            "together",
            "fireworks",
            "perplexity",
            "ollama",
        ] {
            let provider = get_provider(name).unwrap();
            assert_eq!(
                provider.format,
                RequestFormat::OpenAiCompat,
                "Provider {name} should use OpenAiCompat format"
            );
        }
    }

    #[test]
    fn test_get_provider_lm_studio_format() {
        let provider = get_provider("lm_studio").unwrap();
        assert_eq!(provider.format, RequestFormat::LmStudio);
        assert_eq!(provider.api_url, "http://localhost:1234/api/v1/chat");
        assert_eq!(provider.api_headers, "Content-Type: application/json");
        assert_eq!(provider.default_model, "qwen/qwen3.5-35b-a3b");
    }

    #[test]
    fn test_get_provider_ollama_format() {
        let provider = get_provider("ollama").unwrap();
        assert_eq!(provider.format, RequestFormat::OpenAiCompat);
        assert_eq!(
            provider.api_url,
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(provider.api_headers, "Content-Type: application/json");
        assert_eq!(provider.default_model, "llama3");
        assert_eq!(provider.response_path, "choices.0.message.content");
    }

    #[test]
    fn test_default_model_for_known() {
        assert_eq!(default_model_for("groq"), "llama-3.3-70b-versatile");
        assert_eq!(default_model_for("openai"), "gpt-4o-mini");
        assert_eq!(default_model_for("anthropic"), "claude-sonnet-4-20250514");
        assert_eq!(default_model_for("lm_studio"), "qwen/qwen3.5-35b-a3b");
        assert_eq!(default_model_for("ollama"), "llama3");
    }

    #[test]
    fn test_default_model_for_unknown() {
        assert_eq!(default_model_for("custom"), "");
        assert_eq!(default_model_for("unknown"), "");
    }

    #[test]
    fn test_resolve_provider_known() {
        let cfg = AppConfig {
            provider: "groq".into(),
            api_key: "test-key".into(),
            ..Default::default()
        };
        let (url, headers, format, path) = resolve_provider(&cfg).unwrap();
        assert!(url.contains("groq.com"));
        assert!(headers.contains("Bearer"));
        assert_eq!(format, RequestFormat::OpenAiCompat);
        assert_eq!(path, "choices.0.message.content");
    }

    #[test]
    fn test_resolve_provider_known_with_override() {
        let cfg = AppConfig {
            provider: "groq".into(),
            api_url: "https://custom.url/v1".into(),
            api_headers: "X-Custom: value".into(),
            ..Default::default()
        };
        let (url, headers, _, _) = resolve_provider(&cfg).unwrap();
        assert_eq!(url, "https://custom.url/v1");
        assert_eq!(headers, "X-Custom: value");
    }

    #[test]
    fn test_resolve_provider_custom_requires_url() {
        let cfg = AppConfig {
            provider: "custom-provider".into(),
            api_url: "".into(),
            ..Default::default()
        };
        let result = resolve_provider(&cfg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown provider"));
    }

    #[test]
    fn test_resolve_provider_custom_with_url() {
        let cfg = AppConfig {
            provider: "custom-provider".into(),
            api_url: "https://my-custom-api.com/v1".into(),
            api_headers: "Authorization: custom".into(),
            ..Default::default()
        };
        let (url, headers, format, path) = resolve_provider(&cfg).unwrap();
        assert_eq!(url, "https://my-custom-api.com/v1");
        assert_eq!(headers, "Authorization: custom");
        assert_eq!(format, RequestFormat::OpenAiCompat);
        assert_eq!(path, "choices.0.message.content");
    }

    #[test]
    fn test_llm_call_error_display_http() {
        let err = LlmCallError::HttpError {
            code: 401,
            body: "Unauthorized".into(),
        };
        let display = format!("{err}");
        assert!(display.contains("HTTP 401"));
        assert!(display.contains("Unauthorized"));
    }

    #[test]
    fn test_llm_call_error_display_transport() {
        let err = LlmCallError::TransportError("connection refused".into());
        let display = format!("{err}");
        assert!(display.contains("Network error"));
        assert!(display.contains("connection refused"));
    }

    #[test]
    fn test_llm_call_error_display_other() {
        let err = LlmCallError::Other(anyhow::anyhow!("custom error"));
        let display = format!("{err}");
        assert!(display.contains("custom error"));
    }

    #[test]
    fn test_request_format_equality() {
        assert_eq!(RequestFormat::Gemini, RequestFormat::Gemini);
        assert_eq!(RequestFormat::OpenAiCompat, RequestFormat::OpenAiCompat);
        assert_eq!(RequestFormat::Anthropic, RequestFormat::Anthropic);
        assert_eq!(RequestFormat::LmStudio, RequestFormat::LmStudio);
        assert_ne!(RequestFormat::Gemini, RequestFormat::OpenAiCompat);
    }

    #[test]
    fn test_extract_message_lm_studio_message_item() {
        let json = serde_json::json!({
            "output": [
                { "type": "reasoning", "content": "thinking" },
                { "type": "message", "content": "feat: lm studio response" }
            ]
        });
        let result = extract_message(&json, RequestFormat::LmStudio, "output").unwrap();
        assert_eq!(result, "feat: lm studio response");
    }

    #[test]
    fn test_extract_message_lm_studio_missing_message_item() {
        let json = serde_json::json!({
            "output": [
                { "type": "reasoning", "content": "thinking only" }
            ]
        });
        let result = extract_message(&json, RequestFormat::LmStudio, "output");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("type 'message'"));
    }

    #[test]
    fn local_harness_names_and_aliases_are_recognized() {
        for name in [
            "claude_code",
            "claude-code",
            "codex",
            "opencode",
            "gemini_cli",
            "gemini-cli",
        ] {
            assert!(is_local_harness(name), "unrecognized harness {name}");
            assert_eq!(default_model_for(name), "");
        }
        assert!(!is_local_harness("gemini"));
    }

    #[test]
    fn local_harnesses_ignore_http_configuration() {
        let cfg = AppConfig {
            provider: "codex".into(),
            api_url: "https://example.test/v1/chat/completions?key=$ACR_API_KEY".into(),
            api_headers: "Authorization: Bearer $ACR_API_KEY".into(),
            ..Default::default()
        };
        assert!(!provider_requires_api_key(&cfg));
        assert!(model_list_spec(&cfg).is_none());
    }

    #[test]
    fn harness_commands_are_non_interactive_and_read_only() {
        let cfg = AppConfig {
            model: "chosen-model".into(),
            ..Default::default()
        };
        let cases = [
            (Harness::ClaudeCode, "claude", "--permission-mode", "plan"),
            (Harness::Codex, "codex", "--sandbox", "read-only"),
            (Harness::OpenCode, "opencode", "--agent", "plan"),
            (Harness::GeminiCli, "gemini", "--approval-mode", "plan"),
        ];

        for (harness, executable, mode_flag, mode) in cases {
            let mut output = None;
            let command = harness_command(harness, &cfg, &mut output).unwrap();
            let args = command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            assert_eq!(command.get_program(), executable);
            assert!(args.windows(2).any(|pair| pair == [mode_flag, mode]));
            assert!(args
                .windows(2)
                .any(|pair| pair == ["--model", "chosen-model"]));
            assert_eq!(output.is_some(), harness == Harness::Codex);
        }
    }

    #[test]
    fn blank_model_uses_harness_configuration() {
        let cfg = AppConfig {
            model: "  ".into(),
            ..Default::default()
        };
        let mut output = None;
        let command = harness_command(Harness::GeminiCli, &cfg, &mut output).unwrap();
        assert!(!command.get_args().any(|arg| arg == "--model"));
    }

    #[test]
    fn parses_claude_and_gemini_json_output() {
        assert_eq!(
            extract_json_field(
                r#"{"result":"feat: from claude"}"#,
                "result",
                Harness::ClaudeCode
            )
            .unwrap(),
            "feat: from claude"
        );
        assert_eq!(
            extract_json_field(
                r#"{"response":"fix: from gemini"}"#,
                "response",
                Harness::GeminiCli
            )
            .unwrap(),
            "fix: from gemini"
        );
    }

    #[test]
    fn parses_and_deduplicates_opencode_text_events() {
        let output = concat!(
            "{\"type\":\"step_start\"}\n",
            "{\"type\":\"text\",\"part\":{\"id\":\"one\",\"text\":\"feat:\"}}\n",
            "{\"type\":\"text\",\"part\":{\"id\":\"one\",\"text\":\"feat: add\"}}\n",
            "{\"type\":\"text\",\"part\":{\"id\":\"two\",\"text\":\" harnesses\"}}\n"
        );
        assert_eq!(
            extract_opencode_text(output).unwrap(),
            "feat: add harnesses"
        );
    }

    #[test]
    fn combined_prompt_separates_system_instructions_from_user_input() {
        let prompt = combined_harness_prompt("system", "user");
        assert!(prompt.contains("<system_instructions>\nsystem\n</system_instructions>"));
        assert!(prompt.contains("\n\nuser\n\n"));
        assert!(prompt.contains("Do not inspect the workspace or call tools"));
    }
}

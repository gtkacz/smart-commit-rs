use anyhow::{bail, Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;
use std::time::Duration;

use crate::config::AppConfig;
use crate::interpolation::interpolate;

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
        _ => None,
    }
}

/// Get the default model for a built-in provider, or empty string for unknown providers.
pub fn default_model_for(provider: &str) -> &'static str {
    get_provider(provider).map_or("", |p| p.default_model)
}

pub enum LlmCallError {
    HttpError { code: u16, body: String },
    TransportError(String),
    Other(anyhow::Error),
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

fn call_llm_inner(cfg: &AppConfig, system_prompt: &str, diff: &str) -> Result<String, LlmCallError> {
    let (url, headers_raw, format, response_path) =
        resolve_provider(cfg).map_err(LlmCallError::Other)?;

    let url = interpolate(&url, cfg);
    let headers_raw = interpolate(&headers_raw, cfg);

    let body = build_request_body(format, &cfg.model, system_prompt, diff);
    let headers = parse_headers(&headers_raw);

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg} {elapsed}")
            .unwrap(),
    );
    spinner.set_message("Generating commit message...");
    spinner.enable_steady_tick(Duration::from_millis(80));

    let mut req = ureq::post(&url);
    for (key, val) in &headers {
        req = req.set(key, val);
    }
    req = req.set("Content-Type", "application/json");

    let response = req.send_json(&body);

    spinner.finish_and_clear();

    let response = match response {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            return Err(LlmCallError::HttpError { code, body });
        }
        Err(ureq::Error::Transport(t)) => {
            return Err(LlmCallError::TransportError(t.to_string()));
        }
    };

    let json: Value = response
        .into_json()
        .map_err(|e| LlmCallError::Other(anyhow::anyhow!("Failed to parse API response as JSON: {e}")))?;

    let message = extract_message(&json, format, &response_path).map_err(|e| {
        LlmCallError::Other(anyhow::anyhow!(
            "Failed to extract message from response at path '{}'. Response:\n{}\nError: {}",
            response_path,
            serde_json::to_string_pretty(&json).unwrap_or_default(),
            e
        ))
    })?;

    Ok(message)
}

/// Call LLM with fallback support. Returns (message, fallback_preset_name_if_used).
pub fn call_llm_with_fallback(
    cfg: &AppConfig,
    system_prompt: &str,
    diff: &str,
) -> Result<(String, Option<String>)> {
    match call_llm_inner(cfg, system_prompt, diff) {
        Ok(msg) => Ok((msg, None)),
        Err(LlmCallError::TransportError(msg)) => {
            anyhow::bail!("Network error: {msg}");
        }
        Err(LlmCallError::HttpError { code, body }) => {
            if !cfg.fallback_enabled {
                anyhow::bail!("API returned HTTP {code}: {body}");
            }

            let presets_file = match crate::preset::load_presets() {
                Ok(f) => f,
                Err(_) => anyhow::bail!("API returned HTTP {code}: {body}"),
            };

            if presets_file.fallback.order.is_empty() {
                anyhow::bail!("API returned HTTP {code}: {body}");
            }

            let current_fields = crate::preset::fields_from_config(cfg);
            let mut errors = vec![format!("Primary (HTTP {code})")];

            for &preset_id in &presets_file.fallback.order {
                let preset = match presets_file.presets.iter().find(|p| p.id == preset_id) {
                    Some(p) => p,
                    None => continue,
                };

                // Skip if this preset matches current config (dedup key comparison)
                if preset.fields.provider == current_fields.provider
                    && preset.fields.model == current_fields.model
                    && preset.fields.api_key == current_fields.api_key
                    && preset.fields.api_url == current_fields.api_url
                {
                    continue;
                }

                eprintln!(
                    "{} Primary failed (HTTP {}), trying: {}...",
                    "fallback:".yellow().bold(),
                    code,
                    preset.name
                );

                let mut temp_cfg = cfg.clone();
                crate::preset::apply_preset_to_config(&mut temp_cfg, preset);

                match call_llm_inner(&temp_cfg, system_prompt, diff) {
                    Ok(msg) => return Ok((msg, Some(preset.name.clone()))),
                    Err(LlmCallError::HttpError { code: fc, .. }) => {
                        errors.push(format!("{} (HTTP {fc})", preset.name));
                        continue;
                    }
                    Err(LlmCallError::TransportError(msg)) => {
                        anyhow::bail!("Network error during fallback to '{}': {msg}", preset.name);
                    }
                    Err(LlmCallError::Other(e)) => {
                        errors.push(format!("{} ({})", preset.name, e));
                        continue;
                    }
                }
            }

            anyhow::bail!(
                "All LLM providers failed: {}",
                errors.join(", ")
            );
        }
        Err(LlmCallError::Other(e)) => {
            anyhow::bail!("{e}");
        }
    }
}

/// Call the LLM API and return the generated commit message
pub fn call_llm(cfg: &AppConfig, system_prompt: &str, diff: &str) -> Result<String> {
    let (msg, _) = call_llm_with_fallback(cfg, system_prompt, diff)?;
    Ok(msg)
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

fn build_request_body(
    format: RequestFormat,
    model: &str,
    system_prompt: &str,
    diff: &str,
) -> Value {
    match format {
        RequestFormat::Gemini => {
            serde_json::json!({
                "system_instruction": {
                    "parts": [{ "text": system_prompt }]
                },
                "contents": [{
                    "role": "user",
                    "parts": [{ "text": diff }]
                }],
                "generationConfig": {
                    "temperature": 0
                }
            })
        }
        RequestFormat::OpenAiCompat => {
            serde_json::json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": diff }
                ],
                "max_tokens": 512,
                "temperature": 0
            })
        }
        RequestFormat::Anthropic => {
            serde_json::json!({
                "model": model,
                "system": system_prompt,
                "messages": [
                    { "role": "user", "content": diff }
                ],
                "max_tokens": 512
            })
        }
        RequestFormat::LmStudio => {
            serde_json::json!({
                "model": model,
                "input": diff
            })
        }
    }
}

/// Parse "Key: Value, Key2: Value2" header string into pairs
fn parse_headers(raw: &str) -> Vec<(String, String)> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    raw.split(',')
        .filter_map(|pair| {
            let pair = pair.trim();
            pair.split_once(':')
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
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
        assert!(parse_headers("").is_empty());
        assert!(parse_headers("   ").is_empty());
    }

    #[test]
    fn test_parse_headers_single() {
        let headers = parse_headers("Authorization: Bearer abc123");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Authorization");
        assert_eq!(headers[0].1, "Bearer abc123");
    }

    #[test]
    fn test_parse_headers_multiple() {
        let headers = parse_headers("X-Api-Key: key123, Content-Type: application/json");
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].0, "X-Api-Key");
        assert_eq!(headers[0].1, "key123");
        assert_eq!(headers[1].0, "Content-Type");
        assert_eq!(headers[1].1, "application/json");
    }

    #[test]
    fn test_parse_headers_trims_whitespace() {
        let headers = parse_headers("  Key  :  Value  ");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Key");
        assert_eq!(headers[0].1, "Value");
    }

    #[test]
    fn test_parse_headers_skips_invalid() {
        let headers = parse_headers("Valid: Header, InvalidNoColon, Another: One");
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0].0, "Valid");
        assert_eq!(headers[1].0, "Another");
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
        );
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "system prompt");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "user diff");
        assert_eq!(body["max_tokens"], 512);
        assert_eq!(body["temperature"], 0);
    }

    #[test]
    fn test_build_request_body_gemini() {
        let body = build_request_body(
            RequestFormat::Gemini,
            "gemini-pro",
            "system prompt",
            "user diff",
        );
        assert_eq!(body["system_instruction"]["parts"][0]["text"], "system prompt");
        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "user diff");
        assert_eq!(body["generationConfig"]["temperature"], 0);
    }

    #[test]
    fn test_build_request_body_anthropic() {
        let body = build_request_body(
            RequestFormat::Anthropic,
            "claude-3-opus",
            "system prompt",
            "user diff",
        );
        assert_eq!(body["model"], "claude-3-opus");
        assert_eq!(body["system"], "system prompt");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "user diff");
        assert_eq!(body["max_tokens"], 512);
    }

    #[test]
    fn test_build_request_body_lm_studio() {
        let body = build_request_body(
            RequestFormat::LmStudio,
            "qwen/qwen3.5-35b-a3b",
            "system prompt",
            "user diff",
        );
        assert_eq!(body["model"], "qwen/qwen3.5-35b-a3b");
        assert_eq!(body["input"], "user diff");
        assert!(body.get("messages").is_none());
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
        assert!(provider.api_url.contains("generativelanguage.googleapis.com"));
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
        ] {
            let provider = get_provider(name).unwrap();
            assert_eq!(provider.format, RequestFormat::OpenAiCompat, "Provider {name} should use OpenAiCompat format");
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
    fn test_default_model_for_known() {
        assert_eq!(default_model_for("groq"), "llama-3.3-70b-versatile");
        assert_eq!(default_model_for("openai"), "gpt-4o-mini");
        assert_eq!(default_model_for("anthropic"), "claude-sonnet-4-20250514");
        assert_eq!(default_model_for("lm_studio"), "qwen/qwen3.5-35b-a3b");
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("type 'message'"));
    }
}

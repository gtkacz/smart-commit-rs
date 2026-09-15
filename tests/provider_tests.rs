mod common;

use crate::common::EnvGuard;
use auto_commit_rs::config::AppConfig;
use auto_commit_rs::provider;
use mockito::{Matcher, Server};
use serial_test::serial;
use std::fs;

fn cfg_for(provider_name: &str, api_url: String) -> AppConfig {
    AppConfig {
        provider: provider_name.to_string(),
        model: "test-model".into(),
        api_key: "test-key".into(),
        api_url,
        fallback_enabled: false,
        ..Default::default()
    }
}

#[test]
fn default_model_for_returns_known_and_unknown_defaults() {
    assert_eq!(provider::default_model_for("openai"), "gpt-4o-mini");
    assert_eq!(
        provider::default_model_for("groq"),
        "llama-3.3-70b-versatile"
    );
    assert_eq!(provider::default_model_for("unknown"), "");
}

#[test]
fn default_model_for_all_known_providers() {
    assert_eq!(provider::default_model_for("gemini"), "gemini-2.0-flash");
    assert_eq!(
        provider::default_model_for("anthropic"),
        "claude-sonnet-4-20250514"
    );
    assert_eq!(provider::default_model_for("grok"), "grok-3");
    assert_eq!(provider::default_model_for("deepseek"), "deepseek-chat");
    assert_eq!(
        provider::default_model_for("openrouter"),
        "openai/gpt-4o-mini"
    );
    assert_eq!(
        provider::default_model_for("mistral"),
        "mistral-small-latest"
    );
    assert_eq!(
        provider::default_model_for("together"),
        "meta-llama/Llama-3.3-70B-Instruct-Turbo"
    );
    assert_eq!(
        provider::default_model_for("fireworks"),
        "accounts/fireworks/models/llama-v3p3-70b-instruct"
    );
    assert_eq!(provider::default_model_for("perplexity"), "sonar");
    assert_eq!(
        provider::default_model_for("lm_studio"),
        "qwen/qwen3.5-35b-a3b"
    );
}

#[test]
fn provider_requires_api_key_returns_false_for_local_providers() {
    let ollama_cfg = cfg_for(
        "ollama",
        "http://localhost:11434/v1/chat/completions".into(),
    );
    assert!(!provider::provider_requires_api_key(&ollama_cfg));

    let lm_studio_cfg = cfg_for("lm_studio", "http://localhost:1234/api/v1/chat".into());
    assert!(!provider::provider_requires_api_key(&lm_studio_cfg));
}

#[test]
fn provider_requires_api_key_returns_true_for_cloud_providers() {
    let openai_cfg = cfg_for("openai", String::new());
    assert!(provider::provider_requires_api_key(&openai_cfg));

    let gemini_cfg = cfg_for("gemini", String::new());
    assert!(provider::provider_requires_api_key(&gemini_cfg));

    let anthropic_cfg = cfg_for("anthropic", String::new());
    assert!(provider::provider_requires_api_key(&anthropic_cfg));
}

#[test]
fn call_llm_rejects_empty_response_content() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/empty")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":""}}]}"#)
        .create();

    let cfg = cfg_for("openai", format!("{}/empty", server.url()));
    let err = provider::call_llm(&cfg, "system", "diff").expect_err("empty content should fail");
    assert!(err.to_string().contains("empty commit message"));
    mock.assert();
}

#[test]
fn call_llm_rejects_whitespace_only_response_content() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/whitespace")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"   \n  "}}]}"#)
        .create();

    let cfg = cfg_for("openai", format!("{}/whitespace", server.url()));
    let err = provider::call_llm(&cfg, "system", "diff")
        .expect_err("whitespace-only content should fail");
    assert!(err.to_string().contains("empty commit message"));
    mock.assert();
}

#[test]
fn call_llm_openai_compat_builds_expected_request() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/openai")
        .match_header("authorization", "Bearer test-key")
        .match_header("content-type", "application/json")
        .match_body(Matcher::Regex(r#""model":"test-model""#.into()))
        .match_body(Matcher::Regex(r#""messages""#.into()))
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"feat: mocked"}}]}"#)
        .create();

    let cfg = cfg_for("openai", format!("{}/openai", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "feat: mocked");
    mock.assert();
}

#[test]
fn call_llm_gemini_uses_gemini_payload_and_response_path() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/gemini")
        .match_body(Matcher::Regex(r#""system_instruction""#.into()))
        .match_body(Matcher::Regex(r#""generationConfig""#.into()))
        .with_status(200)
        .with_body(r#"{"candidates":[{"content":{"parts":[{"text":"fix: gemini"}]}}]}"#)
        .create();

    let cfg = cfg_for("gemini", format!("{}/gemini", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "fix: gemini");
    mock.assert();
}

#[test]
fn call_llm_anthropic_uses_anthropic_payload_and_headers() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/anthropic")
        .match_header("x-api-key", "test-key")
        .match_header("anthropic-version", "2023-06-01")
        .match_body(Matcher::Regex(r#""system":"system-prompt""#.into()))
        .with_status(200)
        .with_body(r#"{"content":[{"text":"docs: anthropic"}]}"#)
        .create();

    let cfg = cfg_for("anthropic", format!("{}/anthropic", server.url()));
    let msg = provider::call_llm(&cfg, "system-prompt", "diff").expect("llm call");
    assert_eq!(msg, "docs: anthropic");
    mock.assert();
}

#[test]
fn call_llm_custom_provider_requires_url() {
    let cfg = AppConfig {
        provider: "custom-provider".into(),
        api_key: "k".into(),
        ..Default::default()
    };
    let err = provider::call_llm(&cfg, "system", "diff")
        .expect_err("missing custom URL should fail")
        .to_string();
    assert!(err.contains("Unknown provider"));
}

#[test]
#[serial]
fn call_llm_reports_http_status_and_bad_response_path_errors() {
    let mut server = Server::new();
    let status_mock = server
        .mock("POST", "/status")
        .with_status(401)
        .with_body("unauthorized")
        .create();

    let cfg = cfg_for("openai", format!("{}/status", server.url()));
    let status_err = provider::call_llm(&cfg, "system", "diff")
        .expect_err("status failure expected")
        .to_string();
    assert!(status_err.contains("HTTP 401"));
    status_mock.assert();

    let path_mock = server
        .mock("POST", "/missing-path")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"wrong":"value"}}]}"#)
        .create();
    let cfg2 = cfg_for("openai", format!("{}/missing-path", server.url()));
    let path_err = provider::call_llm(&cfg2, "system", "diff")
        .expect_err("missing response path should fail")
        .to_string();
    assert!(path_err.contains("Failed to extract message"));
    path_mock.assert();
}

#[test]
fn call_llm_interpolates_custom_headers_and_url_variables() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", Matcher::Any)
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"chore: custom"}}]}"#)
        .create();

    let mut cfg = cfg_for("custom", format!("{}/v1/$ACR_MODEL", server.url()));
    cfg.api_headers = "Authorization: Bearer $ACR_API_KEY, X-Model: $ACR_MODEL".into();
    cfg.model = "chat".into();

    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "chore: custom");
    mock.assert();
}

#[test]
#[serial]
fn call_llm_with_fallback_tries_next_preset_after_payload_too_large() {
    let mut server = Server::new();

    let mock_primary = server
        .mock("POST", "/primary")
        .with_status(413)
        .with_body("request too large")
        .create();

    // Fallback succeeds
    let mock_fallback = server
        .mock("POST", "/fallback")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"fallback success"}}]}"#)
        .create();

    let mut cfg = cfg_for("custom", format!("{}/primary", server.url()));
    cfg.fallback_enabled = true;

    // Setup presets file
    let cfg_dir = tempfile::TempDir::new().expect("tempdir");
    let _env = EnvGuard::set(&[("ACR_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref())]);

    // Create cgen dir inside config home
    let cgen_dir = cfg_dir.path().join("cgen");
    fs::create_dir_all(&cgen_dir).expect("create cgen dir");

    let fallback_url = format!("{}/fallback", server.url());

    let presets_toml = format!(
        r#"
next_id = 1
[[presets]]
id = 0
name = "fallback-preset"
provider = "custom"
model = "fallback-model"
api_key = "fallback-key"
api_url = "{}"
api_headers = ""

[fallback]
enabled = true
order = [0]
"#,
        fallback_url
    );

    fs::write(cgen_dir.join("presets.toml"), presets_toml).expect("write presets");

    let (msg, preset_name) =
        provider::call_llm_with_fallback(&cfg, "system", "diff").expect("llm call with fallback");

    assert_eq!(msg, "fallback success");
    assert_eq!(preset_name, Some("fallback-preset".to_string()));

    mock_primary.assert();
    mock_fallback.assert();
}

#[test]
#[serial]
fn call_llm_with_fallback_tries_preset_differing_only_in_headers() {
    let mut server = Server::new();

    // Primary (no X-Route header) fails with a retryable status
    let mock_primary = server
        .mock("POST", "/shared")
        .match_header("x-route", Matcher::Missing)
        .with_status(500)
        .with_body("server error")
        .create();

    // Same URL: the preset differs from the active config only by a header
    let mock_fallback = server
        .mock("POST", "/shared")
        .match_header("x-route", "fallback")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"fix: header fallback"}}]}"#)
        .create();

    let mut cfg = cfg_for("custom", format!("{}/shared", server.url()));
    cfg.fallback_enabled = true;

    let cfg_dir = tempfile::TempDir::new().expect("tempdir");
    let _env = EnvGuard::set(&[("ACR_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref())]);
    let cgen_dir = cfg_dir.path().join("cgen");
    fs::create_dir_all(&cgen_dir).expect("create cgen dir");

    let presets_toml = format!(
        r#"
next_id = 1
[[presets]]
id = 0
name = "header-preset"
provider = "custom"
model = "test-model"
api_key = "test-key"
api_url = "{}/shared"
api_headers = "X-Route: fallback"

[fallback]
order = [0]
"#,
        server.url()
    );
    fs::write(cgen_dir.join("presets.toml"), presets_toml).expect("write presets");

    let (msg, preset_name) = provider::call_llm_with_fallback(&cfg, "system", "diff")
        .expect("preset differing only in headers should be attempted");

    assert_eq!(msg, "fix: header fallback");
    assert_eq!(preset_name, Some("header-preset".to_string()));
    mock_primary.assert();
    mock_fallback.assert();
}

#[test]
fn call_llm_deepseek_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/deepseek")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"deepseek response"}}]}"#)
        .create();

    let cfg = cfg_for("deepseek", format!("{}/deepseek", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "deepseek response");
    mock.assert();
}

#[test]
fn call_llm_grok_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/grok")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"grok response"}}]}"#)
        .create();

    let cfg = cfg_for("grok", format!("{}/grok", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "grok response");
    mock.assert();
}

#[test]
fn call_llm_openrouter_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/openrouter")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"openrouter response"}}]}"#)
        .create();

    let cfg = cfg_for("openrouter", format!("{}/openrouter", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "openrouter response");
    mock.assert();
}

#[test]
fn call_llm_mistral_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/mistral")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"mistral response"}}]}"#)
        .create();

    let cfg = cfg_for("mistral", format!("{}/mistral", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "mistral response");
    mock.assert();
}

#[test]
fn call_llm_together_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/together")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"together response"}}]}"#)
        .create();

    let cfg = cfg_for("together", format!("{}/together", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "together response");
    mock.assert();
}

#[test]
fn call_llm_fireworks_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/fireworks")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"fireworks response"}}]}"#)
        .create();

    let cfg = cfg_for("fireworks", format!("{}/fireworks", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "fireworks response");
    mock.assert();
}

#[test]
fn call_llm_perplexity_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/perplexity")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"perplexity response"}}]}"#)
        .create();

    let cfg = cfg_for("perplexity", format!("{}/perplexity", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "perplexity response");
    mock.assert();
}

#[test]
fn call_llm_lm_studio_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/lm-studio")
        .match_header("content-type", "application/json")
        .match_body(Matcher::Regex(r#""model":"test-model""#.into()))
        .match_body(Matcher::Regex(r#""input":"diff""#.into()))
        .with_status(200)
        .with_body(
            r#"{
                "model_instance_id":"abc",
                "output":[
                    {"type":"reasoning","content":"thinking"},
                    {"type":"message","content":"lm studio response"}
                ],
                "stats":{"tokens":123},
                "response_id":"res_1"
            }"#,
        )
        .create();

    let cfg = cfg_for("lm_studio", format!("{}/lm-studio", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "lm studio response");
    mock.assert();
}

#[test]
fn generate_validated_message_retries_with_validator_feedback() {
    let mut server = Server::new();
    // Exact-body match so this mock can never swallow the correction request
    let initial_body = serde_json::json!({
        "model": "test-model",
        "messages": [
            { "role": "system", "content": "system" },
            {
                "role": "user",
                "content": "<diff>\ndiff\n</diff>\n\nWrite the commit message for the staged changes in <diff>, following all rules you were given. Output only the raw commit message."
            }
        ],
        "max_tokens": 512,
        "temperature": 0,
        "stream": false
    });
    let initial_mock = server
        .mock("POST", "/generate")
        .match_body(Matcher::Json(initial_body))
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"not a conventional message"}}]}"#)
        .expect(1)
        .create();
    let correction_mock = server
        .mock("POST", "/generate")
        .match_body(Matcher::Regex("previous_attempt".into()))
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"fix: corrected message"}}]}"#)
        .expect(1)
        .create();

    let cfg = cfg_for("openai", format!("{}/generate", server.url()));
    let (message, fallback) =
        provider::generate_validated_message(&cfg, "system", "diff").expect("generation");

    assert_eq!(message, "fix: corrected message");
    assert!(fallback.is_none());
    initial_mock.assert();
    correction_mock.assert();
}

#[test]
fn generate_validated_message_fails_when_correction_is_also_invalid() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/generate")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"still not conventional"}}]}"#)
        .expect(2)
        .create();

    let cfg = cfg_for("openai", format!("{}/generate", server.url()));
    let error = provider::generate_validated_message(&cfg, "system", "diff")
        .expect_err("both attempts invalid");

    assert!(error.to_string().contains("corrective retry"));
    mock.assert();
}

#[test]
fn call_llm_groq_provider() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/groq")
        .match_header("authorization", "Bearer test-key")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"groq response"}}]}"#)
        .create();

    let cfg = cfg_for("groq", format!("{}/groq", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "groq response");
    mock.assert();
}

#[test]
fn call_llm_with_custom_url_overrides_provider_default() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/custom-endpoint")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"custom endpoint response"}}]}"#)
        .create();

    let cfg = cfg_for("openai", format!("{}/custom-endpoint", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "custom endpoint response");
    mock.assert();
}

#[test]
fn call_llm_empty_headers_for_provider_with_defaults() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/gemini")
        .with_status(200)
        .with_body(r#"{"candidates":[{"content":{"parts":[{"text":"gemini no headers"}]}}]}"#)
        .create();

    let cfg = cfg_for("gemini", format!("{}/gemini", server.url()));
    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "gemini no headers");
    mock.assert();
}

#[test]
#[serial]
fn call_llm_fallback_disabled_returns_error() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/fail")
        .with_status(500)
        .with_body("internal error")
        .create();

    let mut cfg = cfg_for("custom", format!("{}/fail", server.url()));
    cfg.fallback_enabled = false;

    let result = provider::call_llm(&cfg, "system", "diff");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTP 500"));
    mock.assert();
}

#[test]
#[serial]
fn call_llm_fallback_empty_order_returns_error() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/fail")
        .with_status(401)
        .with_body("unauthorized")
        .create();

    let cfg_dir = tempfile::TempDir::new().expect("tempdir");
    let _env = EnvGuard::set(&[("ACR_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref())]);

    // Create empty presets file
    let cgen_dir = cfg_dir.path().join("cgen");
    fs::create_dir_all(&cgen_dir).expect("create cgen dir");
    fs::write(
        cgen_dir.join("presets.toml"),
        "next_id = 0\npresets = []\n[fallback]\nenabled = true\norder = []\n",
    )
    .expect("write presets");

    let mut cfg = cfg_for("custom", format!("{}/fail", server.url()));
    cfg.fallback_enabled = true;

    let result = provider::call_llm(&cfg, "system", "diff");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTP 401"));
    mock.assert();
}

#[test]
fn call_llm_invalid_json_response() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/invalid")
        .with_status(200)
        .with_body("not json at all")
        .create();

    let cfg = cfg_for("openai", format!("{}/invalid", server.url()));
    let result = provider::call_llm(&cfg, "system", "diff");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to parse API response"));
    mock.assert();
}

#[test]
fn call_llm_with_multiple_custom_headers() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/headers")
        .match_header("X-Custom-One", "value1")
        .match_header("X-Custom-Two", "value2")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"headers ok"}}]}"#)
        .create();

    let mut cfg = cfg_for("custom", format!("{}/headers", server.url()));
    cfg.api_headers = "X-Custom-One: value1, X-Custom-Two: value2".into();

    let msg = provider::call_llm(&cfg, "system", "diff").expect("llm call");
    assert_eq!(msg, "headers ok");
    mock.assert();
}

#[cfg(unix)]
fn install_fake_harnesses(directory: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let script = r#"#!/bin/sh
input=$(cat)
if [ -n "$HARNESS_PROMPT_LOG" ]; then
    printf '%s' "$input" > "$HARNESS_PROMPT_LOG"
fi
if [ -n "$HARNESS_FAIL" ]; then
    printf 'rejected top-secret' >&2
    exit 9
fi
case "$HARNESS_MODE" in
    bad-json) printf 'not json'; exit 0 ;;
    missing-field) printf '{}'; exit 0 ;;
    empty) printf '{"result":""}'; exit 0 ;;
    stdout-fail) printf 'failed top-secret'; exit 9 ;;
    silent-fail) exit 9 ;;
esac
name=${0##*/}
case "$name" in
    claude) printf '{"result":"feat: claude harness"}' ;;
    codex)
        while [ "$#" -gt 0 ]; do
            if [ "$1" = "--output-last-message" ]; then
                shift
                printf 'feat: codex harness' > "$1"
                break
            fi
            shift
        done
        ;;
    opencode) printf '%s\n' '{"type":"text","part":{"id":"one","text":"feat: opencode harness"}}' ;;
    gemini) printf '{"response":"feat: gemini harness"}' ;;
esac
"#;

    for executable in ["claude", "codex", "opencode", "gemini"] {
        let path = directory.join(executable);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[cfg(unix)]
#[test]
#[serial]
fn local_harnesses_execute_end_to_end_and_redact_failures() {
    let directory = tempfile::tempdir().unwrap();
    install_fake_harnesses(directory.path());
    let prompt_log = directory.path().join("prompt.log");
    let path = std::env::join_paths(std::iter::once(directory.path().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    let path = path.to_string_lossy().into_owned();
    let prompt_log_value = prompt_log.to_string_lossy().into_owned();
    let _env = EnvGuard::set(&[("PATH", &path), ("HARNESS_PROMPT_LOG", &prompt_log_value)]);

    for (provider_name, expected) in [
        ("claude_code", "feat: claude harness"),
        ("codex", "feat: codex harness"),
        ("opencode", "feat: opencode harness"),
        ("gemini_cli", "feat: gemini harness"),
    ] {
        let cfg = AppConfig {
            provider: provider_name.into(),
            model: String::new(),
            api_key: "top-secret".into(),
            fallback_enabled: false,
            ..Default::default()
        };
        let (message, fallback) = provider::call_llm_with_fallback_mode(
            &cfg,
            "system instructions",
            "user prompt",
            provider::OutputMode::Quiet,
        )
        .unwrap();
        assert_eq!(message, expected);
        assert_eq!(fallback, None);
        let prompt = fs::read_to_string(&prompt_log).unwrap();
        assert!(prompt.contains("system instructions"));
        assert!(prompt.contains("user prompt"));
    }

    let cfg = AppConfig {
        provider: "claude_code".into(),
        model: String::new(),
        api_key: "top-secret".into(),
        fallback_enabled: false,
        ..Default::default()
    };
    let call = || {
        provider::call_llm_with_fallback_mode(
            &cfg,
            "system instructions",
            "user prompt",
            provider::OutputMode::Quiet,
        )
        .unwrap_err()
        .to_string()
    };

    {
        let _failure = EnvGuard::set(&[("HARNESS_FAIL", "1")]);
        let error = call();
        assert!(error.contains("exit status: 9"));
        assert!(error.contains("[REDACTED]"));
        assert!(!error.contains("top-secret"));
    }
    {
        let _mode = EnvGuard::set(&[("HARNESS_MODE", "stdout-fail")]);
        let error = call();
        assert!(error.contains("[REDACTED]"));
        assert!(!error.contains("top-secret"));
    }
    {
        let _mode = EnvGuard::set(&[("HARNESS_MODE", "silent-fail")]);
        assert!(call().contains("no diagnostic output"));
    }
    for (mode, expected) in [
        ("bad-json", "parse Claude Code output"),
        ("missing-field", "did not contain a string 'result' field"),
        ("empty", "empty commit message"),
    ] {
        let _mode = EnvGuard::set(&[("HARNESS_MODE", mode)]);
        assert!(call().contains(expected));
    }
    {
        let _mode = EnvGuard::set(&[("HARNESS_MODE", "bad-json")]);
        let mut opencode_cfg = cfg.clone();
        opencode_cfg.provider = "opencode".into();
        let error = provider::call_llm_with_fallback_mode(
            &opencode_cfg,
            "system instructions",
            "user prompt",
            provider::OutputMode::Quiet,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("parse OpenCode JSON event"));
    }

    let missing_directory = tempfile::tempdir().unwrap();
    let missing_path = missing_directory.path().to_string_lossy();
    let _missing_path = EnvGuard::set(&[("PATH", &missing_path)]);
    assert!(call().contains("Failed to start Claude Code executable"));
}

mod common;

use auto_commit_rs::config::AppConfig;
use auto_commit_rs::provider;
use crate::common::EnvGuard;
use mockito::{Matcher, Server};
use serial_test::serial;
use std::fs;

fn cfg_for(provider_name: &str, api_url: String) -> AppConfig {
    let mut cfg = AppConfig::default();
    cfg.provider = provider_name.to_string();
    cfg.model = "test-model".into();
    cfg.api_key = "test-key".into();
    cfg.api_url = api_url;
    cfg
}

#[test]
fn default_model_for_returns_known_and_unknown_defaults() {
    assert_eq!(provider::default_model_for("openai"), "gpt-4o-mini");
    assert_eq!(provider::default_model_for("groq"), "llama-3.3-70b-versatile");
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
    let mut cfg = AppConfig::default();
    cfg.provider = "custom-provider".into();
    cfg.api_url.clear();
    cfg.api_key = "k".into();
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
fn call_llm_with_fallback_tries_next_preset() {
    let mut server = Server::new();
    
    // Primary fails
    let mock_primary = server
        .mock("POST", "/primary")
        .with_status(500)
        .with_body("server error")
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
    let _env = EnvGuard::set(&[
        ("ACR_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
    ]);
    
    // Create cgen dir inside config home
    let cgen_dir = cfg_dir.path().join("cgen");
    fs::create_dir_all(&cgen_dir).expect("create cgen dir");

    let fallback_url = format!("{}/fallback", server.url());
    
    let presets_toml = format!(r#"
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
"#, fallback_url);

    fs::write(cgen_dir.join("presets.toml"), presets_toml).expect("write presets");

    let (msg, preset_name) = provider::call_llm_with_fallback(&cfg, "system", "diff").expect("llm call with fallback");
    
    assert_eq!(msg, "fallback success");
    assert_eq!(preset_name, Some("fallback-preset".to_string()));
    
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
    let _env = EnvGuard::set(&[(
        "ACR_CONFIG_HOME",
        cfg_dir.path().to_string_lossy().as_ref(),
    )]);

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

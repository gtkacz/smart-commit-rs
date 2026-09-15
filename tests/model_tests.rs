use auto_commit_rs::config::AppConfig;
use auto_commit_rs::{model, provider};
use mockito::Matcher;

fn config(api_url: String) -> AppConfig {
    AppConfig {
        provider: "openai".into(),
        model: "current".into(),
        api_key: "top-secret-key".into(),
        api_url,
        api_headers: "Authorization: Bearer $ACR_API_KEY".into(),
        ..AppConfig::default()
    }
}

#[test]
fn discovers_models_from_derived_authenticated_endpoint() {
    let mut server = mockito::Server::new();
    let request = server
        .mock("GET", "/v1/models")
        .match_header(
            "authorization",
            Matcher::Exact("Bearer top-secret-key".into()),
        )
        .with_status(200)
        .with_body(r#"{"data":[{"id":"z-model"},{"id":"a-model"},{"id":"a-model"}]}"#)
        .create();
    let cfg = config(format!("{}/v1/chat/completions", server.url()));

    assert_eq!(
        model::discover_models(&cfg).unwrap(),
        vec!["a-model", "z-model"]
    );
    request.assert();
}

#[test]
fn model_discovery_errors_redact_credentials() {
    let mut server = mockito::Server::new();
    let request = server
        .mock("GET", "/v1/models")
        .with_status(401)
        .with_body("rejected top-secret-key")
        .create();
    let cfg = config(format!("{}/v1/chat/completions", server.url()));

    let error = model::discover_models(&cfg).unwrap_err().to_string();
    assert!(error.contains("[REDACTED]"));
    assert!(!error.contains("top-secret-key"));
    request.assert();
}

#[test]
fn arbitrary_custom_endpoints_and_perplexity_use_manual_fallback() {
    let custom = AppConfig {
        provider: "custom".into(),
        api_url: "https://example.test/generate".into(),
        ..AppConfig::default()
    };
    assert!(provider::model_list_spec(&custom).is_none());

    let perplexity = AppConfig {
        provider: "perplexity".into(),
        api_url: String::new(),
        ..AppConfig::default()
    };
    assert!(provider::model_list_spec(&perplexity).is_none());

    for provider_name in ["claude_code", "codex", "opencode", "gemini_cli"] {
        let harness = AppConfig {
            provider: provider_name.into(),
            api_url: "https://ignored.test/v1/chat/completions".into(),
            ..AppConfig::default()
        };
        assert!(provider::model_list_spec(&harness).is_none());
    }
}

#[test]
fn built_in_model_metadata_covers_supported_provider_families() {
    for provider_name in [
        "openai",
        "anthropic",
        "gemini",
        "groq",
        "grok",
        "deepseek",
        "openrouter",
        "mistral",
        "together",
        "fireworks",
        "lm_studio",
        "ollama",
    ] {
        let cfg = AppConfig {
            provider: provider_name.into(),
            api_url: String::new(),
            ..AppConfig::default()
        };
        assert!(
            provider::model_list_spec(&cfg).is_some(),
            "missing metadata for {provider_name}"
        );
    }
}

#[test]
fn rejects_oversized_model_responses() {
    let mut server = mockito::Server::new();
    let request = server
        .mock("GET", "/v1/models")
        .with_status(200)
        .with_body(vec![b'x'; 1_048_577])
        .create();
    let cfg = config(format!("{}/v1/chat/completions", server.url()));

    let error = model::discover_models(&cfg).unwrap_err().to_string();
    assert!(error.contains("safety limit"));
    request.assert();
}

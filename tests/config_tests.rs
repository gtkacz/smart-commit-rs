mod common;

use std::fs;

use auto_commit_rs::config::{field_description, global_config_path, AppConfig};
use serial_test::serial;

use crate::common::{DirGuard, EnvGuard, GlobalConfigGuard};

fn acr_env_keys() -> [&'static str; 18] {
    [
        "ACR_CONFIG_HOME",
        "ACR_PROVIDER",
        "ACR_MODEL",
        "ACR_API_KEY",
        "ACR_API_URL",
        "ACR_API_HEADERS",
        "ACR_LOCALE",
        "ACR_ONE_LINER",
        "ACR_COMMIT_TEMPLATE",
        "ACR_LLM_SYSTEM_PROMPT",
        "ACR_USE_GITMOJI",
        "ACR_GITMOJI_FORMAT",
        "ACR_REVIEW_COMMIT",
        "ACR_POST_COMMIT_PUSH",
        "ACR_SUPPRESS_TOOL_OUTPUT",
        "ACR_WARN_STAGED_FILES_ENABLED",
        "ACR_WARN_STAGED_FILES_THRESHOLD",
        "ACR_CONFIRM_NEW_VERSION",
    ]
}

#[test]
#[serial]
fn load_uses_defaults_when_no_layers_exist() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let _global = GlobalConfigGuard::backup();
    let cfg_dir = tempfile::TempDir::new().expect("tempdir");

    let _env = EnvGuard::set(&[
        ("ACR_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("XDG_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("APPDATA", cfg_dir.path().to_string_lossy().as_ref()),
    ]);
    let _acr = EnvGuard::clear(&acr_env_keys());

    let _force = EnvGuard::set(&[
        ("ACR_PROVIDER", "groq"),
        ("ACR_MODEL", "llama-3.3-70b-versatile"),
        ("ACR_LOCALE", "en"),
        ("ACR_POST_COMMIT_PUSH", "ask"),
        ("ACR_WARN_STAGED_FILES_THRESHOLD", "20"),
        ("ACR_CONFIRM_NEW_VERSION", "1"),
    ]);
    let cfg = AppConfig::load().expect("config should load");
    assert_eq!(cfg.provider, "groq");
    assert_eq!(cfg.model, "llama-3.3-70b-versatile");
    assert_eq!(cfg.locale, "en");
    assert!(cfg.one_liner);
    assert_eq!(cfg.post_commit_push, "ask");
    assert_eq!(cfg.warn_staged_files_threshold, 20);
    assert!(cfg.confirm_new_version);
}

#[test]
#[serial]
fn load_applies_global_then_local_then_env_precedence() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let _global = GlobalConfigGuard::backup();
    let cfg_dir = tempfile::TempDir::new().expect("tempdir");

    let _env = EnvGuard::set(&[
        ("ACR_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("XDG_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("APPDATA", cfg_dir.path().to_string_lossy().as_ref()),
    ]);
    let _acr = EnvGuard::clear(&acr_env_keys());

    let global_path = global_config_path().expect("global path");
    fs::create_dir_all(
        global_path
            .parent()
            .expect("global config parent should be present"),
    )
    .expect("create global config dir");
    fs::write(
        &global_path,
        r#"
provider = "openai"
model = "gpt-4o-mini"
locale = "en"
post_commit_push = "always"
warn_staged_files_threshold = 7
"#,
    )
    .expect("write global config");

    fs::write(
        repo.path().join(".env"),
        r#"
# local override
ACR_PROVIDER=anthropic
ACR_MODEL=claude-local
ACR_POST_COMMIT_PUSH=never
ACR_WARN_STAGED_FILES_THRESHOLD=13
ACR_API_HEADERS='X-Foo: bar'
"#,
    )
    .expect("write local env");

    let _env_overrides = EnvGuard::set(&[
        ("ACR_MODEL", "env-model"),
        ("ACR_POST_COMMIT_PUSH", "invalid-value"),
        ("ACR_WARN_STAGED_FILES_THRESHOLD", "not-a-number"),
        ("ACR_USE_GITMOJI", "true"),
    ]);

    let cfg = AppConfig::load().expect("config should load");
    assert_eq!(cfg.provider, "anthropic");
    assert_eq!(cfg.model, "env-model");
    assert_eq!(cfg.post_commit_push, "ask");
    assert_eq!(cfg.warn_staged_files_threshold, 20);
    assert!(cfg.use_gitmoji);
    assert_eq!(cfg.api_headers, "X-Foo: bar");
}

#[test]
#[serial]
fn save_local_writes_normalized_env_file() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let mut cfg = AppConfig::default();
    cfg.provider = "gemini".into();
    cfg.model = "gemini-2.0-flash".into();
    cfg.api_key = "secret-key-value".into();
    cfg.locale = "pl".into();
    cfg.post_commit_push = "unexpected".into();
    cfg.warn_staged_files_enabled = false;
    cfg.warn_staged_files_threshold = 42;
    cfg.confirm_new_version = false;

    cfg.save_local().expect("save local config");

    let env_content = fs::read_to_string(repo.path().join(".env")).expect("read .env");
    assert!(env_content.contains("ACR_PROVIDER=gemini"));
    assert!(env_content.contains("ACR_MODEL=gemini-2.0-flash"));
    assert!(env_content.contains("ACR_API_KEY=secret-key-value"));
    assert!(env_content.contains("ACR_POST_COMMIT_PUSH=ask"));
    assert!(env_content.contains("ACR_WARN_STAGED_FILES_ENABLED=0"));
    assert!(env_content.contains("ACR_WARN_STAGED_FILES_THRESHOLD=42"));
    assert!(env_content.contains("ACR_CONFIRM_NEW_VERSION=0"));
}

#[test]
fn set_field_parses_boolean_and_numeric_values() {
    let mut cfg = AppConfig::default();
    cfg.set_field("ONE_LINER", "0").expect("set one-liner");
    cfg.set_field("USE_GITMOJI", "true")
        .expect("set use gitmoji");
    cfg.set_field("WARN_STAGED_FILES_THRESHOLD", "15")
        .expect("set warning threshold");
    cfg.set_field("WARN_STAGED_FILES_THRESHOLD", "invalid")
        .expect("set invalid warning threshold");
    cfg.set_field("POST_COMMIT_PUSH", "ALWAYS")
        .expect("set post commit push");

    assert!(!cfg.one_liner);
    assert!(cfg.use_gitmoji);
    assert_eq!(cfg.warn_staged_files_threshold, 20);
    assert_eq!(cfg.post_commit_push, "always");
}

#[test]
#[serial]
fn load_errors_when_locale_has_no_i18n_resources() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let _global = GlobalConfigGuard::backup();
    let cfg_dir = tempfile::TempDir::new().expect("tempdir");

    let _env = EnvGuard::set(&[
        ("ACR_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("XDG_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("APPDATA", cfg_dir.path().to_string_lossy().as_ref()),
    ]);
    let _acr = EnvGuard::clear(&acr_env_keys());
    let _set_locale = EnvGuard::set(&[("ACR_LOCALE", "pl")]);

    let err = AppConfig::load().expect_err("expected locale validation error");
    assert!(
        err.to_string().contains("Unsupported locale"),
        "unexpected error: {err:#}"
    );
}

#[test]
#[serial]
fn load_accepts_non_english_locale_when_i18n_exists() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let _global = GlobalConfigGuard::backup();
    let cfg_dir = tempfile::TempDir::new().expect("tempdir");

    let _env = EnvGuard::set(&[
        ("ACR_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("XDG_CONFIG_HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("HOME", cfg_dir.path().to_string_lossy().as_ref()),
        ("APPDATA", cfg_dir.path().to_string_lossy().as_ref()),
    ]);
    let _acr = EnvGuard::clear(&acr_env_keys());
    let _set_locale = EnvGuard::set(&[("ACR_LOCALE", "pl")]);

    fs::create_dir_all(repo.path().join("i18n")).expect("create i18n dir");
    fs::write(repo.path().join("i18n").join("pl.toml"), "title = 'Polski'")
        .expect("write locale resource");

    let cfg = AppConfig::load().expect("config should load with i18n locale");
    assert_eq!(cfg.locale, "pl");
}

#[test]
fn fields_display_masks_api_key_and_shows_helpers() {
    let mut cfg = AppConfig::default();
    cfg.api_key = "abcd1234efgh5678".into();
    cfg.api_url.clear();
    cfg.api_headers.clear();

    let fields = cfg.fields_display();
    let api_key = fields
        .iter()
        .find(|(name, _, _)| *name == "API Key")
        .expect("api key field");
    let api_url = fields
        .iter()
        .find(|(name, _, _)| *name == "API URL")
        .expect("api url field");

    assert_eq!(api_key.2, "abcd...5678");
    assert_eq!(api_url.2, "(auto from provider)");
}

#[test]
fn fields_display_shows_enabled_disabled_for_booleans() {
    let mut cfg = AppConfig::default();
    cfg.one_liner = true;
    cfg.use_gitmoji = false;
    cfg.review_commit = true;
    cfg.suppress_tool_output = false;
    cfg.warn_staged_files_enabled = true;
    cfg.confirm_new_version = false;
    cfg.auto_update = Some(true);

    let fields = cfg.fields_display();

    let find_val = |name: &str| -> String {
        fields
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, _, v)| v.clone())
            .unwrap()
    };

    assert_eq!(find_val("One-liner"), "enabled");
    assert_eq!(find_val("Use Gitmoji"), "disabled");
    assert_eq!(find_val("Review Commit"), "enabled");
    assert_eq!(find_val("Suppress Tool Output"), "disabled");
    assert_eq!(find_val("Warn Staged Files"), "enabled");
    assert_eq!(find_val("Confirm New Version"), "disabled");
    assert_eq!(find_val("Auto Update"), "enabled");

    // Verify the inverse
    cfg.one_liner = false;
    cfg.auto_update = Some(false);
    let fields = cfg.fields_display();
    let find_val2 = |name: &str| -> String {
        fields
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, _, v)| v.clone())
            .unwrap()
    };
    assert_eq!(find_val2("One-liner"), "disabled");
    assert_eq!(find_val2("Auto Update"), "disabled");
}

#[test]
fn fields_display_auto_update_none_shows_not_set() {
    let mut cfg = AppConfig::default();
    cfg.auto_update = None;

    let fields = cfg.fields_display();
    let auto_update = fields
        .iter()
        .find(|(name, _, _)| *name == "Auto Update")
        .expect("auto update field");

    assert_eq!(auto_update.2, "(not set)");
}

#[test]
fn grouped_fields_has_basic_and_advanced_groups() {
    let cfg = AppConfig::default();
    let groups = cfg.grouped_fields();

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].name, "Basic");
    assert_eq!(groups[1].name, "Advanced");

    // Basic has 4 direct fields
    assert_eq!(groups[0].fields.len(), 4);
    let basic_suffixes: Vec<&str> = groups[0].fields.iter().map(|(_, s, _)| *s).collect();
    assert_eq!(basic_suffixes, vec!["PROVIDER", "MODEL", "API_KEY", "API_URL"]);

    // Advanced has 4 subgroups
    assert_eq!(groups[1].subgroups.len(), 4);
    assert_eq!(groups[1].subgroups[0].name, "LLM Settings");
    assert_eq!(groups[1].subgroups[1].name, "Commit Behavior");
    assert_eq!(groups[1].subgroups[2].name, "Post-Commit");
    assert_eq!(groups[1].subgroups[3].name, "Warnings & Updates");
}

#[test]
fn grouped_fields_covers_all_config_fields() {
    let cfg = AppConfig::default();
    let groups = cfg.grouped_fields();
    let flat_fields = cfg.fields_display();

    let mut grouped_suffixes: Vec<&str> = Vec::new();
    for group in &groups {
        for (_, suffix, _) in &group.fields {
            grouped_suffixes.push(suffix);
        }
        for sg in &group.subgroups {
            for (_, suffix, _) in &sg.fields {
                grouped_suffixes.push(suffix);
            }
        }
    }

    let flat_suffixes: Vec<&str> = flat_fields.iter().map(|(_, s, _)| *s).collect();
    assert_eq!(grouped_suffixes.len(), flat_suffixes.len());
    for suffix in &flat_suffixes {
        assert!(
            grouped_suffixes.contains(suffix),
            "suffix {} missing from grouped fields",
            suffix
        );
    }
}

#[test]
fn field_description_returns_descriptions_for_known_fields() {
    assert!(!field_description("PROVIDER").is_empty());
    assert!(!field_description("MODEL").is_empty());
    assert!(!field_description("API_KEY").is_empty());
    assert!(!field_description("API_URL").is_empty());
    assert!(!field_description("LOCALE").is_empty());
    assert!(!field_description("ONE_LINER").is_empty());
    assert!(!field_description("USE_GITMOJI").is_empty());
    assert!(!field_description("FALLBACK_ENABLED").is_empty());
    assert!(!field_description("TRACK_GENERATED_COMMITS").is_empty());

    // Unknown fields return empty string
    assert!(field_description("UNKNOWN_FIELD").is_empty());
    assert!(field_description("").is_empty());
}

#[test]
fn set_field_all_boolean_fields() {
    let mut cfg = AppConfig::default();

    cfg.set_field("ONE_LINER", "true").unwrap();
    assert!(cfg.one_liner);
    cfg.set_field("ONE_LINER", "false").unwrap();
    assert!(!cfg.one_liner);
    cfg.set_field("ONE_LINER", "1").unwrap();
    assert!(cfg.one_liner);

    cfg.set_field("USE_GITMOJI", "TRUE").unwrap();
    assert!(cfg.use_gitmoji);

    cfg.set_field("REVIEW_COMMIT", "1").unwrap();
    assert!(cfg.review_commit);

    cfg.set_field("SUPPRESS_TOOL_OUTPUT", "true").unwrap();
    assert!(cfg.suppress_tool_output);

    cfg.set_field("WARN_STAGED_FILES_ENABLED", "false").unwrap();
    assert!(!cfg.warn_staged_files_enabled);

    cfg.set_field("CONFIRM_NEW_VERSION", "0").unwrap();
    assert!(!cfg.confirm_new_version);

    cfg.set_field("FALLBACK_ENABLED", "false").unwrap();
    assert!(!cfg.fallback_enabled);

    cfg.set_field("TRACK_GENERATED_COMMITS", "0").unwrap();
    assert!(!cfg.track_generated_commits);
}

#[test]
fn set_field_string_fields() {
    let mut cfg = AppConfig::default();

    cfg.set_field("PROVIDER", "anthropic").unwrap();
    assert_eq!(cfg.provider, "anthropic");

    cfg.set_field("MODEL", "claude-3.5").unwrap();
    assert_eq!(cfg.model, "claude-3.5");

    cfg.set_field("API_KEY", "sk-test-key").unwrap();
    assert_eq!(cfg.api_key, "sk-test-key");

    cfg.set_field("API_URL", "https://api.example.com").unwrap();
    assert_eq!(cfg.api_url, "https://api.example.com");

    cfg.set_field("API_HEADERS", "X-Custom: value").unwrap();
    assert_eq!(cfg.api_headers, "X-Custom: value");

    cfg.set_field("COMMIT_TEMPLATE", "[$msg]").unwrap();
    assert_eq!(cfg.commit_template, "[$msg]");

    cfg.set_field("LLM_SYSTEM_PROMPT", "Custom prompt").unwrap();
    assert_eq!(cfg.llm_system_prompt, "Custom prompt");

    cfg.set_field("GITMOJI_FORMAT", "shortcode").unwrap();
    assert_eq!(cfg.gitmoji_format, "shortcode");
}

#[test]
fn set_field_post_commit_push_normalization() {
    let mut cfg = AppConfig::default();

    cfg.set_field("POST_COMMIT_PUSH", "never").unwrap();
    assert_eq!(cfg.post_commit_push, "never");

    cfg.set_field("POST_COMMIT_PUSH", "ALWAYS").unwrap();
    assert_eq!(cfg.post_commit_push, "always");

    cfg.set_field("POST_COMMIT_PUSH", "Ask").unwrap();
    assert_eq!(cfg.post_commit_push, "ask");

    cfg.set_field("POST_COMMIT_PUSH", "invalid").unwrap();
    assert_eq!(cfg.post_commit_push, "ask");
}

#[test]
fn set_field_diff_exclude_globs() {
    let mut cfg = AppConfig::default();

    cfg.set_field("DIFF_EXCLUDE_GLOBS", "*.json, *.lock, *.svg")
        .unwrap();
    assert_eq!(
        cfg.diff_exclude_globs,
        vec!["*.json", "*.lock", "*.svg"]
    );

    cfg.set_field("DIFF_EXCLUDE_GLOBS", "").unwrap();
    assert!(cfg.diff_exclude_globs.is_empty());
}

#[test]
fn set_field_auto_update() {
    let mut cfg = AppConfig::default();

    cfg.set_field("AUTO_UPDATE", "true").unwrap();
    assert_eq!(cfg.auto_update, Some(true));

    cfg.set_field("AUTO_UPDATE", "false").unwrap();
    assert_eq!(cfg.auto_update, Some(false));

    cfg.set_field("AUTO_UPDATE", "1").unwrap();
    assert_eq!(cfg.auto_update, Some(true));
}

#[test]
fn set_field_unknown_field_is_ignored() {
    let mut cfg = AppConfig::default();
    let original = cfg.provider.clone();

    cfg.set_field("UNKNOWN_FIELD", "value").unwrap();
    assert_eq!(cfg.provider, original);
}

#[test]
fn app_config_default_values() {
    let cfg = AppConfig::default();

    assert_eq!(cfg.provider, "groq");
    assert_eq!(cfg.model, "llama-3.3-70b-versatile");
    assert!(cfg.api_key.is_empty());
    assert!(cfg.api_url.is_empty());
    assert!(cfg.api_headers.is_empty());
    assert_eq!(cfg.locale, "en");
    assert!(cfg.one_liner);
    assert_eq!(cfg.commit_template, "$msg");
    assert!(!cfg.use_gitmoji);
    assert_eq!(cfg.gitmoji_format, "unicode");
    assert!(cfg.review_commit);
    assert_eq!(cfg.post_commit_push, "ask");
    assert!(!cfg.suppress_tool_output);
    assert!(cfg.warn_staged_files_enabled);
    assert_eq!(cfg.warn_staged_files_threshold, 20);
    assert!(cfg.confirm_new_version);
    assert!(cfg.auto_update.is_none());
    assert!(cfg.fallback_enabled);
    assert!(cfg.track_generated_commits);
    assert!(!cfg.diff_exclude_globs.is_empty());
}

#[test]
fn fields_display_diff_exclude_globs_none() {
    let mut cfg = AppConfig::default();
    cfg.diff_exclude_globs.clear();

    let fields = cfg.fields_display();
    let globs = fields
        .iter()
        .find(|(name, _, _)| *name == "Diff Exclude Globs")
        .expect("diff exclude globs field");

    assert_eq!(globs.2, "(none)");
}

#[test]
fn fields_display_api_key_short() {
    let mut cfg = AppConfig::default();
    cfg.api_key = "short".into();

    let fields = cfg.fields_display();
    let key = fields
        .iter()
        .find(|(name, _, _)| *name == "API Key")
        .expect("api key field");

    // Short keys are fully masked
    assert_eq!(key.2, "*****");
}

#[test]
fn fields_display_api_key_empty() {
    let mut cfg = AppConfig::default();
    cfg.api_key.clear();

    let fields = cfg.fields_display();
    let key = fields
        .iter()
        .find(|(name, _, _)| *name == "API Key")
        .expect("api key field");

    assert_eq!(key.2, "(not set)");
}

#[test]
#[serial]
fn save_global_creates_directory_and_file() {
    let cfg_dir = tempfile::TempDir::new().expect("tempdir");
    let _env = EnvGuard::set(&[(
        "ACR_CONFIG_HOME",
        cfg_dir.path().to_string_lossy().as_ref(),
    )]);

    let cfg = AppConfig::default();
    cfg.save_global().expect("save global should succeed");

    let config_path = cfg_dir.path().join("cgen").join("config.toml");
    assert!(config_path.exists());

    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(content.contains("provider = \"groq\""));
}

#[test]
fn field_description_all_fields() {
    let descriptions = [
        "PROVIDER",
        "MODEL",
        "API_KEY",
        "API_URL",
        "API_HEADERS",
        "LOCALE",
        "ONE_LINER",
        "COMMIT_TEMPLATE",
        "LLM_SYSTEM_PROMPT",
        "USE_GITMOJI",
        "GITMOJI_FORMAT",
        "REVIEW_COMMIT",
        "POST_COMMIT_PUSH",
        "SUPPRESS_TOOL_OUTPUT",
        "WARN_STAGED_FILES_ENABLED",
        "WARN_STAGED_FILES_THRESHOLD",
        "CONFIRM_NEW_VERSION",
        "AUTO_UPDATE",
        "FALLBACK_ENABLED",
        "TRACK_GENERATED_COMMITS",
        "DIFF_EXCLUDE_GLOBS",
    ];

    for suffix in descriptions {
        let desc = field_description(suffix);
        assert!(
            !desc.is_empty(),
            "field_description should return non-empty for {}",
            suffix
        );
    }
}

use auto_commit_rs::config::AppConfig;
use auto_commit_rs::prompt::build_system_prompt;

#[test]
fn prompt_includes_core_sections_by_default() {
    let cfg = AppConfig::default();
    let prompt = build_system_prompt(&cfg);

    assert!(prompt.contains("following the Conventional Commits specification"));
    assert!(prompt.contains("single sentence"));
    assert!(prompt.contains("Output only the raw commit message"));
    assert!(!prompt.contains("Use Gitmoji"));
    assert!(!prompt.contains("locale."));
}

#[test]
fn prompt_includes_unicode_gitmoji_when_enabled() {
    let mut cfg = AppConfig::default();
    cfg.use_gitmoji = true;
    cfg.gitmoji_format = "unicode".into();

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.contains("relevant emoji in unicode format"));
    assert!(prompt.contains("⚡"));
}

#[test]
fn prompt_includes_shortcode_gitmoji_and_locale_when_configured() {
    let mut cfg = AppConfig::default();
    cfg.use_gitmoji = true;
    cfg.gitmoji_format = "shortcode".into();
    cfg.locale = "pl".into();
    cfg.one_liner = false;

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.contains("relevant emoji in :shortcode: format"));
    assert!(prompt.contains("Write the commit message in the 'pl' locale."));
    assert!(!prompt.contains("Output ONLY a single line."));
}

#[test]
fn prompt_gitmoji_does_not_override_conventional_commits() {
    let mut cfg = AppConfig::default();
    cfg.use_gitmoji = true;
    cfg.gitmoji_format = "unicode".into();

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.contains("following the Conventional Commits specification"));
    assert!(prompt.contains("Conventional Commits specification above"));
    assert!(prompt.contains("type(scope):"));
    assert!(prompt.contains("feat(api):"));
    assert!(prompt.contains("fix(auth):"));
}

#[test]
fn prompt_uses_custom_base_prompt() {
    let mut cfg = AppConfig::default();
    cfg.llm_system_prompt = "custom base prompt".into();

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.starts_with("custom base prompt"));
}

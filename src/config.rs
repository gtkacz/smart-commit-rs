use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct FieldSubgroup {
    pub name: &'static str,
    pub fields: Vec<(&'static str, &'static str, String)>,
}

pub struct FieldGroup {
    pub name: &'static str,
    pub fields: Vec<(&'static str, &'static str, String)>,
    pub subgroups: Vec<FieldSubgroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_headers: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_true")]
    pub one_liner: bool,
    #[serde(default = "default_commit_template")]
    pub commit_template: String,
    #[serde(default = "default_system_prompt")]
    pub llm_system_prompt: String,
    #[serde(default)]
    pub use_gitmoji: bool,
    #[serde(default = "default_gitmoji_format")]
    pub gitmoji_format: String,
    #[serde(default)]
    pub review_commit: bool,
    #[serde(default = "default_post_commit_push")]
    pub post_commit_push: String,
    #[serde(default)]
    pub suppress_tool_output: bool,
    #[serde(default = "default_true")]
    pub warn_staged_files_enabled: bool,
    #[serde(default = "default_warn_staged_files_threshold")]
    pub warn_staged_files_threshold: usize,
    #[serde(default = "default_true")]
    pub warn_llm_files_enabled: bool,
    #[serde(default = "default_warn_llm_files_threshold")]
    pub warn_llm_files_threshold: usize,
    #[serde(default = "default_true")]
    pub confirm_new_version: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
    #[serde(default = "default_true")]
    pub fallback_enabled: bool,
    #[serde(default = "default_true")]
    pub track_generated_commits: bool,
    #[serde(default = "default_diff_exclude_globs")]
    pub diff_exclude_globs: Vec<String>,
    #[serde(default = "default_max_diff_bytes")]
    pub max_diff_bytes: usize,
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: usize,
    #[serde(default = "default_sensitive_file_globs")]
    pub sensitive_file_globs: Vec<String>,
}

/// A global configuration file is an overlay, not a second set of defaults.
/// Keeping every field optional is what lets omitted TOML keys inherit the
/// application default instead of deserializing as `false` or an empty value.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartialAppConfig {
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    api_url: Option<String>,
    api_headers: Option<String>,
    locale: Option<String>,
    one_liner: Option<bool>,
    commit_template: Option<String>,
    llm_system_prompt: Option<String>,
    use_gitmoji: Option<bool>,
    gitmoji_format: Option<String>,
    review_commit: Option<bool>,
    post_commit_push: Option<String>,
    suppress_tool_output: Option<bool>,
    warn_staged_files_enabled: Option<bool>,
    warn_staged_files_threshold: Option<usize>,
    warn_llm_files_enabled: Option<bool>,
    warn_llm_files_threshold: Option<usize>,
    confirm_new_version: Option<bool>,
    auto_update: Option<bool>,
    fallback_enabled: Option<bool>,
    track_generated_commits: Option<bool>,
    diff_exclude_globs: Option<Vec<String>>,
    max_diff_bytes: Option<usize>,
    max_output_tokens: Option<usize>,
    sensitive_file_globs: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct LocalConfigState {
    pub config: AppConfig,
    pub inherited: AppConfig,
    pub explicit_fields: HashSet<String>,
}

fn default_provider() -> String {
    "groq".into()
}
fn default_model() -> String {
    "llama-3.3-70b-versatile".into()
}
fn default_locale() -> String {
    "en".into()
}
pub fn default_true() -> bool {
    true
}
fn default_post_commit_push() -> String {
    "ask".into()
}
fn default_commit_template() -> String {
    "$msg".into()
}
fn default_system_prompt() -> String {
    // Blank means "use the built-in default", so prompt upgrades reach configs
    // that never customized it (crate::prompt resolves the actual text)
    String::new()
}
fn default_gitmoji_format() -> String {
    "unicode".into()
}
fn default_warn_staged_files_threshold() -> usize {
    20
}
fn default_warn_llm_files_threshold() -> usize {
    20
}
fn default_diff_exclude_globs() -> Vec<String> {
    vec![
        "*.json",
        "*.xml",
        "*.csv",
        "*.pdf",
        "*.lock",
        "*.svg",
        "*.png",
        "*.jpg",
        "*.jpeg",
        "*.gif",
        "*.ico",
        "*.woff",
        "*.woff2",
        "*.ttf",
        "*.eot",
        "*.min.js",
        "*.min.css",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
fn default_max_diff_bytes() -> usize {
    200_000
}
fn default_max_output_tokens() -> usize {
    512
}
fn default_sensitive_file_globs() -> Vec<String> {
    [
        ".env",
        ".env.*",
        "*.pem",
        "*.key",
        "id_rsa",
        "id_ed25519",
        "*credentials*",
        "*secrets*",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            api_key: String::new(),
            api_url: String::new(),
            api_headers: String::new(),
            locale: default_locale(),
            one_liner: true,
            commit_template: default_commit_template(),
            llm_system_prompt: default_system_prompt(),
            use_gitmoji: false,
            gitmoji_format: default_gitmoji_format(),
            review_commit: true,
            post_commit_push: default_post_commit_push(),
            suppress_tool_output: false,
            warn_staged_files_enabled: true,
            warn_staged_files_threshold: default_warn_staged_files_threshold(),
            warn_llm_files_enabled: true,
            warn_llm_files_threshold: default_warn_llm_files_threshold(),
            confirm_new_version: true,
            auto_update: None,
            fallback_enabled: true,
            track_generated_commits: true,
            diff_exclude_globs: default_diff_exclude_globs(),
            max_diff_bytes: default_max_diff_bytes(),
            max_output_tokens: default_max_output_tokens(),
            sensitive_file_globs: default_sensitive_file_globs(),
        }
    }
}

/// Map of ACR_ env var suffix → struct field name
const ENV_FIELD_MAP: &[(&str, &str)] = &[
    ("PROVIDER", "provider"),
    ("MODEL", "model"),
    ("API_KEY", "api_key"),
    ("API_URL", "api_url"),
    ("API_HEADERS", "api_headers"),
    ("LOCALE", "locale"),
    ("ONE_LINER", "one_liner"),
    ("COMMIT_TEMPLATE", "commit_template"),
    ("LLM_SYSTEM_PROMPT", "llm_system_prompt"),
    ("USE_GITMOJI", "use_gitmoji"),
    ("GITMOJI_FORMAT", "gitmoji_format"),
    ("REVIEW_COMMIT", "review_commit"),
    ("POST_COMMIT_PUSH", "post_commit_push"),
    ("SUPPRESS_TOOL_OUTPUT", "suppress_tool_output"),
    ("WARN_STAGED_FILES_ENABLED", "warn_staged_files_enabled"),
    ("WARN_STAGED_FILES_THRESHOLD", "warn_staged_files_threshold"),
    ("WARN_LLM_FILES_ENABLED", "warn_llm_files_enabled"),
    ("WARN_LLM_FILES_THRESHOLD", "warn_llm_files_threshold"),
    ("CONFIRM_NEW_VERSION", "confirm_new_version"),
    ("AUTO_UPDATE", "auto_update"),
    ("FALLBACK_ENABLED", "fallback_enabled"),
    ("TRACK_GENERATED_COMMITS", "track_generated_commits"),
    ("DIFF_EXCLUDE_GLOBS", "diff_exclude_globs"),
    ("MAX_DIFF_BYTES", "max_diff_bytes"),
    ("MAX_OUTPUT_TOKENS", "max_output_tokens"),
    ("SENSITIVE_FILE_GLOBS", "sensitive_file_globs"),
];

impl AppConfig {
    /// Load config with layered resolution: defaults → global TOML → local .env → env vars
    pub fn load() -> Result<Self> {
        let mut cfg = Self::load_global_for_edit()?;

        // Layer 2: Local .env (in git repo root)
        if let Some(env_path) = local_env_path() {
            if env_path.exists() {
                let env_map = parse_dotenv(&env_path)?;
                cfg.apply_env_map(&env_map, true)?;
            }
        }

        // Layer 3: Actual environment variables
        let mut env_map = HashMap::new();
        for (suffix, _) in ENV_FIELD_MAP {
            let key = format!("ACR_{suffix}");
            if let Ok(val) = std::env::var(&key) {
                env_map.insert(key, val);
            }
        }
        cfg.apply_env_map(&env_map, false)?;
        cfg.validate()?;

        Ok(cfg)
    }

    /// Load exactly the defaults and global file, without project or process
    /// overlays. This prevents an editor save from materializing ephemeral values.
    pub fn load_global_for_edit() -> Result<Self> {
        let mut cfg = Self::default();
        if let Some(path) = global_config_path() {
            if path.exists() {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                let partial: PartialAppConfig = toml::from_str(&content)
                    .with_context(|| format!("Failed to parse {}", path.display()))?;
                cfg.apply_partial(partial);
            }
        }
        cfg.validate()?;
        Ok(cfg)
    }

    /// Load the project overlay together with the values it inherits and the
    /// exact fields explicitly present in `.env`.
    pub fn load_local_for_edit() -> Result<LocalConfigState> {
        let inherited = Self::load_global_for_edit()?;
        let mut config = inherited.clone();
        let mut explicit_fields = HashSet::new();

        if let Some(path) = local_env_path() {
            if path.exists() {
                let map = parse_dotenv(&path)?;
                for (suffix, _) in ENV_FIELD_MAP {
                    if *suffix != "AUTO_UPDATE" && map.contains_key(&format!("ACR_{suffix}")) {
                        explicit_fields.insert((*suffix).to_string());
                    }
                }
                config.apply_env_map(&map, true)?;
            }
        }
        config.validate()?;
        Ok(LocalConfigState {
            config,
            inherited,
            explicit_fields,
        })
    }

    fn apply_partial(&mut self, partial: PartialAppConfig) {
        macro_rules! apply {
            ($($field:ident),+ $(,)?) => {
                $(if let Some(value) = partial.$field {
                    self.$field = value;
                })+
            };
        }
        apply!(
            provider,
            model,
            api_key,
            api_url,
            api_headers,
            locale,
            one_liner,
            commit_template,
            llm_system_prompt,
            use_gitmoji,
            gitmoji_format,
            review_commit,
            post_commit_push,
            suppress_tool_output,
            warn_staged_files_enabled,
            warn_staged_files_threshold,
            warn_llm_files_enabled,
            warn_llm_files_threshold,
            confirm_new_version,
            fallback_enabled,
            track_generated_commits,
            diff_exclude_globs,
            max_diff_bytes,
            max_output_tokens,
            sensitive_file_globs,
        );
        if partial.auto_update.is_some() {
            self.auto_update = partial.auto_update;
        }
    }

    fn apply_env_map(&mut self, map: &HashMap<String, String>, from_local: bool) -> Result<()> {
        for (suffix, _field) in ENV_FIELD_MAP {
            let key = format!("ACR_{suffix}");
            if let Some(val) = map.get(&key) {
                if *suffix == "AUTO_UPDATE" && from_local {
                    continue;
                }
                self.set_field(suffix, val)
                    .with_context(|| format!("Invalid value for {key}"))?;
            }
        }
        Ok(())
    }

    /// Save to global TOML config file
    pub fn save_global(&self) -> Result<()> {
        let path = global_config_path().context("Could not determine global config directory")?;
        let mut validated = self.clone();
        validated.validate()?;
        let content = toml::to_string_pretty(&validated).context("Failed to serialize config")?;
        crate::persistence::atomic_write(&path, content.as_bytes())?;
        Ok(())
    }

    /// Save every supported project field as an explicit local override.
    ///
    /// Interactive editing uses [`save_local_overrides`] instead so inherited
    /// values remain absent. This full form remains for callers that explicitly
    /// want to materialize the current configuration.
    pub fn save_local(&self) -> Result<()> {
        let explicit_fields = ENV_FIELD_MAP
            .iter()
            .filter(|(suffix, _)| *suffix != "AUTO_UPDATE")
            .map(|(suffix, _)| (*suffix).to_string())
            .collect();
        self.save_local_overrides(&explicit_fields)
    }

    /// Persist only explicit local overrides while leaving all unrelated `.env`
    /// content byte-for-byte intact.
    pub fn save_local_overrides(&self, explicit_fields: &HashSet<String>) -> Result<()> {
        let mut validated = self.clone();
        validated.validate()?;
        let env_path = local_env_path().context("Not in a git repository")?;
        crate::persistence::with_file_lock(&env_path, || {
            let original = match std::fs::read_to_string(&env_path) {
                Ok(content) => content,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("Failed to read {}", env_path.display()))
                }
            };
            let rewritten = rewrite_dotenv(&original, &validated, explicit_fields)?;
            crate::persistence::atomic_write_unlocked(&env_path, rewritten.as_bytes())
        })
    }

    /// Get all fields as (display_name, env_suffix, current_value) tuples
    pub fn fields_display(&self) -> Vec<(&'static str, &'static str, String)> {
        vec![
            ("Provider", "PROVIDER", self.provider.clone()),
            ("Model", "MODEL", self.model.clone()),
            (
                "API Key",
                "API_KEY",
                if self.api_key.is_empty() {
                    "(not set)".into()
                } else {
                    mask_key(&self.api_key)
                },
            ),
            (
                "API URL",
                "API_URL",
                if self.api_url.is_empty() {
                    "(auto from provider)".into()
                } else {
                    self.api_url.clone()
                },
            ),
            (
                "API Headers",
                "API_HEADERS",
                if self.api_headers.is_empty() {
                    "(auto from provider)".into()
                } else {
                    self.api_headers.clone()
                },
            ),
            ("Locale", "LOCALE", self.locale.clone()),
            (
                "One-liner",
                "ONE_LINER",
                if self.one_liner {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "Commit Template",
                "COMMIT_TEMPLATE",
                self.commit_template.clone(),
            ),
            (
                "System Prompt",
                "LLM_SYSTEM_PROMPT",
                if crate::prompt::base_prompt_is_default(&self.llm_system_prompt) {
                    "(built-in default)".into()
                } else {
                    truncate(&self.llm_system_prompt, 60)
                },
            ),
            (
                "Use Gitmoji",
                "USE_GITMOJI",
                if self.use_gitmoji {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "Gitmoji Format",
                "GITMOJI_FORMAT",
                self.gitmoji_format.clone(),
            ),
            (
                "Review Commit",
                "REVIEW_COMMIT",
                if self.review_commit {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "Post Commit Push",
                "POST_COMMIT_PUSH",
                normalize_post_commit_push(&self.post_commit_push),
            ),
            (
                "Suppress Tool Output",
                "SUPPRESS_TOOL_OUTPUT",
                if self.suppress_tool_output {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "Warn Staged Files",
                "WARN_STAGED_FILES_ENABLED",
                if self.warn_staged_files_enabled {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "Staged Warn Threshold",
                "WARN_STAGED_FILES_THRESHOLD",
                self.warn_staged_files_threshold.to_string(),
            ),
            (
                "Warn LLM Files",
                "WARN_LLM_FILES_ENABLED",
                if self.warn_llm_files_enabled {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "LLM Warn Threshold",
                "WARN_LLM_FILES_THRESHOLD",
                self.warn_llm_files_threshold.to_string(),
            ),
            (
                "Confirm New Version",
                "CONFIRM_NEW_VERSION",
                if self.confirm_new_version {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "Auto Update",
                "AUTO_UPDATE",
                match self.auto_update {
                    Some(true) => "enabled".into(),
                    Some(false) => "disabled".into(),
                    None => "(not set)".into(),
                },
            ),
            (
                "Fallback Enabled",
                "FALLBACK_ENABLED",
                if self.fallback_enabled {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "Track Generated Commits",
                "TRACK_GENERATED_COMMITS",
                if self.track_generated_commits {
                    "enabled".into()
                } else {
                    "disabled".into()
                },
            ),
            (
                "Diff Exclude Globs",
                "DIFF_EXCLUDE_GLOBS",
                if self.diff_exclude_globs.is_empty() {
                    "(none)".into()
                } else {
                    self.diff_exclude_globs.join(", ")
                },
            ),
            (
                "Max Diff Bytes",
                "MAX_DIFF_BYTES",
                self.max_diff_bytes.to_string(),
            ),
            (
                "Max Output Tokens",
                "MAX_OUTPUT_TOKENS",
                self.max_output_tokens.to_string(),
            ),
            (
                "Sensitive File Globs",
                "SENSITIVE_FILE_GLOBS",
                self.sensitive_file_globs.join(", "),
            ),
        ]
    }

    /// Field groups for the interactive config UI
    pub fn grouped_fields(&self) -> Vec<FieldGroup> {
        let fields = self.fields_display();
        let field_map: std::collections::HashMap<&str, (&'static str, String)> = fields
            .iter()
            .map(|(name, suffix, val)| (*suffix, (*name, val.clone())))
            .collect();

        let basic_keys: &[&'static str] = &["PROVIDER", "MODEL", "API_KEY", "API_URL"];
        let llm_keys: &[&'static str] = &[
            "API_HEADERS",
            "LOCALE",
            "LLM_SYSTEM_PROMPT",
            "COMMIT_TEMPLATE",
            "FALLBACK_ENABLED",
            "DIFF_EXCLUDE_GLOBS",
            "MAX_DIFF_BYTES",
            "MAX_OUTPUT_TOKENS",
            "SENSITIVE_FILE_GLOBS",
        ];
        let commit_keys: &[&'static str] = &[
            "ONE_LINER",
            "USE_GITMOJI",
            "GITMOJI_FORMAT",
            "REVIEW_COMMIT",
            "TRACK_GENERATED_COMMITS",
        ];
        let post_commit_keys: &[&'static str] = &["POST_COMMIT_PUSH", "SUPPRESS_TOOL_OUTPUT"];
        let warnings_keys: &[&'static str] = &[
            "WARN_STAGED_FILES_ENABLED",
            "WARN_STAGED_FILES_THRESHOLD",
            "WARN_LLM_FILES_ENABLED",
            "WARN_LLM_FILES_THRESHOLD",
            "CONFIRM_NEW_VERSION",
            "AUTO_UPDATE",
        ];

        let collect = |keys: &[&'static str]| -> Vec<(&'static str, &'static str, String)> {
            keys.iter()
                .filter_map(|k| field_map.get(k).map(|(name, val)| (*name, *k, val.clone())))
                .collect()
        };

        vec![
            FieldGroup {
                name: "Basic",
                fields: collect(basic_keys),
                subgroups: vec![],
            },
            FieldGroup {
                name: "Advanced",
                fields: vec![],
                subgroups: vec![
                    FieldSubgroup {
                        name: "LLM Settings",
                        fields: collect(llm_keys),
                    },
                    FieldSubgroup {
                        name: "Commit Behavior",
                        fields: collect(commit_keys),
                    },
                    FieldSubgroup {
                        name: "Post-Commit",
                        fields: collect(post_commit_keys),
                    },
                    FieldSubgroup {
                        name: "Warnings & Updates",
                        fields: collect(warnings_keys),
                    },
                ],
            },
        ]
    }

    /// Set a field by its env suffix
    pub fn set_field(&mut self, suffix: &str, value: &str) -> Result<()> {
        match suffix {
            "PROVIDER" => {
                let provider = value.trim().to_ascii_lowercase();
                if provider.is_empty() {
                    anyhow::bail!("Provider cannot be empty");
                }
                self.provider = provider;
            }
            "MODEL" => self.model = value.into(),
            "API_KEY" => self.api_key = value.into(),
            "API_URL" => self.api_url = value.into(),
            "API_HEADERS" => {
                validate_api_headers(value)?;
                self.api_headers = value.into();
            }
            "LOCALE" => {
                let locale = normalize_locale(value);
                validate_locale(&locale)?;
                self.locale = locale;
            }
            "ONE_LINER" => self.one_liner = parse_bool(value)?,
            "COMMIT_TEMPLATE" => {
                validate_commit_template(value)?;
                self.commit_template = value.into();
            }
            "LLM_SYSTEM_PROMPT" => self.llm_system_prompt = value.into(),
            "USE_GITMOJI" => self.use_gitmoji = parse_bool(value)?,
            "GITMOJI_FORMAT" => {
                self.gitmoji_format = validate_gitmoji_format(value)?;
            }
            "REVIEW_COMMIT" => self.review_commit = parse_bool(value)?,
            "POST_COMMIT_PUSH" => {
                self.post_commit_push = validate_post_commit_push(value)?;
            }
            "SUPPRESS_TOOL_OUTPUT" => self.suppress_tool_output = parse_bool(value)?,
            "WARN_STAGED_FILES_ENABLED" => {
                self.warn_staged_files_enabled = parse_bool(value)?;
            }
            "WARN_STAGED_FILES_THRESHOLD" => {
                self.warn_staged_files_threshold = parse_usize(value)?;
            }
            "WARN_LLM_FILES_ENABLED" => {
                self.warn_llm_files_enabled = parse_bool(value)?;
            }
            "WARN_LLM_FILES_THRESHOLD" => {
                self.warn_llm_files_threshold = parse_usize(value)?;
            }
            "CONFIRM_NEW_VERSION" => {
                self.confirm_new_version = parse_bool(value)?;
            }
            "AUTO_UPDATE" => {
                self.auto_update = Some(parse_bool(value)?);
            }
            "FALLBACK_ENABLED" => {
                self.fallback_enabled = parse_bool(value)?;
            }
            "TRACK_GENERATED_COMMITS" => {
                self.track_generated_commits = parse_bool(value)?;
            }
            "DIFF_EXCLUDE_GLOBS" => {
                let globs: Vec<String> = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                validate_globs(&globs)?;
                self.diff_exclude_globs = globs;
            }
            "MAX_DIFF_BYTES" => {
                self.max_diff_bytes = parse_positive_usize(value, "Max diff bytes")?;
            }
            "MAX_OUTPUT_TOKENS" => {
                self.max_output_tokens = parse_positive_usize(value, "Max output tokens")?;
            }
            "SENSITIVE_FILE_GLOBS" => {
                let globs: Vec<String> = value
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect();
                validate_globs(&globs)?;
                self.sensitive_file_globs = globs;
            }
            _ => anyhow::bail!("Unknown setting '{suffix}'"),
        }
        Ok(())
    }

    /// Copy a single effective value from another configuration layer.
    pub fn inherit_field(&mut self, suffix: &str, inherited: &Self) -> Result<()> {
        let value = inherited.env_value(suffix)?;
        self.set_field(suffix, &value)
    }

    pub fn env_value(&self, suffix: &str) -> Result<String> {
        let bool_value = |value: bool| if value { "1" } else { "0" }.to_string();
        let value = match suffix {
            "PROVIDER" => self.provider.clone(),
            "MODEL" => self.model.clone(),
            "API_KEY" => self.api_key.clone(),
            "API_URL" => self.api_url.clone(),
            "API_HEADERS" => self.api_headers.clone(),
            "LOCALE" => self.locale.clone(),
            "ONE_LINER" => bool_value(self.one_liner),
            "COMMIT_TEMPLATE" => self.commit_template.clone(),
            "LLM_SYSTEM_PROMPT" => self.llm_system_prompt.clone(),
            "USE_GITMOJI" => bool_value(self.use_gitmoji),
            "GITMOJI_FORMAT" => self.gitmoji_format.clone(),
            "REVIEW_COMMIT" => bool_value(self.review_commit),
            "POST_COMMIT_PUSH" => self.post_commit_push.clone(),
            "SUPPRESS_TOOL_OUTPUT" => bool_value(self.suppress_tool_output),
            "WARN_STAGED_FILES_ENABLED" => bool_value(self.warn_staged_files_enabled),
            "WARN_STAGED_FILES_THRESHOLD" => self.warn_staged_files_threshold.to_string(),
            "WARN_LLM_FILES_ENABLED" => bool_value(self.warn_llm_files_enabled),
            "WARN_LLM_FILES_THRESHOLD" => self.warn_llm_files_threshold.to_string(),
            "CONFIRM_NEW_VERSION" => bool_value(self.confirm_new_version),
            "AUTO_UPDATE" => self.auto_update.map(bool_value).unwrap_or_default(),
            "FALLBACK_ENABLED" => bool_value(self.fallback_enabled),
            "TRACK_GENERATED_COMMITS" => bool_value(self.track_generated_commits),
            "DIFF_EXCLUDE_GLOBS" => self.diff_exclude_globs.join(","),
            "MAX_DIFF_BYTES" => self.max_diff_bytes.to_string(),
            "MAX_OUTPUT_TOKENS" => self.max_output_tokens.to_string(),
            "SENSITIVE_FILE_GLOBS" => self.sensitive_file_globs.join(","),
            _ => anyhow::bail!("Unknown setting '{suffix}'"),
        };
        Ok(value)
    }

    /// Apply ephemeral `--set KEY=VALUE` overrides as the highest-priority layer.
    /// Never persisted. Errors on malformed entries, unknown keys, or `auto_update`.
    pub fn apply_overrides(&mut self, overrides: &[String]) -> Result<()> {
        for entry in overrides {
            let (raw_key, value) = entry
                .split_once('=')
                .with_context(|| format!("Invalid --set '{entry}'. Expected KEY=VALUE."))?;
            let suffix = normalize_override_key(raw_key);
            if suffix == "AUTO_UPDATE" {
                anyhow::bail!(
                    "'auto_update' cannot be overridden per-invocation; it is a persistent global preference. Use `cgen config`."
                );
            }
            if !ENV_FIELD_MAP.iter().any(|(s, _)| *s == suffix) {
                anyhow::bail!(
                    "Unknown setting '{raw_key}'. Valid keys: {}",
                    overridable_keys().join(", ")
                );
            }
            self.set_field(&suffix, value)
                .with_context(|| format!("Failed to apply --set {raw_key}={value}"))?;
        }
        Ok(())
    }

    fn validate(&mut self) -> Result<()> {
        self.provider = self.provider.trim().to_ascii_lowercase();
        if self.provider.is_empty() {
            anyhow::bail!("Provider cannot be empty");
        }
        self.locale = normalize_locale(&self.locale);
        validate_locale(&self.locale)?;
        self.post_commit_push = validate_post_commit_push(&self.post_commit_push)?;
        self.gitmoji_format = validate_gitmoji_format(&self.gitmoji_format)?;
        validate_commit_template(&self.commit_template)?;
        validate_globs(&self.diff_exclude_globs)?;
        validate_globs(&self.sensitive_file_globs)?;
        if self.max_diff_bytes == 0 {
            anyhow::bail!("Max diff bytes must be greater than zero");
        }
        if self.max_output_tokens == 0 {
            anyhow::bail!("Max output tokens must be greater than zero");
        }
        validate_api_headers(&self.api_headers)?;
        Ok(())
    }
}

/// Global config file path
pub fn global_config_path() -> Option<PathBuf> {
    if let Some(override_dir) = std::env::var_os("ACR_CONFIG_HOME") {
        let override_path = PathBuf::from(override_dir);
        if !override_path.as_os_str().is_empty() {
            return Some(override_path.join("cgen").join("config.toml"));
        }
    }
    dirs::config_dir().map(|d| d.join("cgen").join("config.toml"))
}

fn local_env_path() -> Option<PathBuf> {
    crate::git::find_repo_root()
        .ok()
        .map(|root| PathBuf::from(root).join(".env"))
}

/// Save only the auto_update preference to global config without overwriting other fields
pub fn save_auto_update_preference(value: bool) -> Result<()> {
    let path = global_config_path().context("Could not determine global config directory")?;
    crate::persistence::with_file_lock(&path, || {
        let mut table: toml::Table = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            content
                .parse()
                .with_context(|| format!("Failed to parse {}", path.display()))?
        } else {
            toml::Table::new()
        };

        table.insert("auto_update".to_string(), toml::Value::Boolean(value));
        let content = toml::to_string_pretty(&table).context("Failed to serialize config")?;
        crate::persistence::atomic_write_unlocked(&path, content.as_bytes())
    })
}

/// Update only the existing global model field, preserving every other
/// configuration value.
pub fn save_global_model(model: &str) -> Result<()> {
    let model = model.trim();
    if model.is_empty() {
        anyhow::bail!("Model cannot be empty");
    }
    let path = global_config_path().context("Could not determine global config directory")?;
    crate::persistence::with_file_lock(&path, || {
        let mut table: toml::Table = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            content
                .parse()
                .with_context(|| format!("Failed to parse {}", path.display()))?
        } else {
            toml::Table::new()
        };
        table.insert("model".into(), toml::Value::String(model.to_string()));
        let content = toml::to_string_pretty(&table).context("Failed to serialize config")?;
        crate::persistence::atomic_write_unlocked(&path, content.as_bytes())
    })
}

fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        "*".repeat(chars.len())
    } else {
        format!(
            "{}...{}",
            chars[..4].iter().collect::<String>(),
            chars[chars.len() - 4..].iter().collect::<String>()
        )
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max).collect::<String>())
    }
}

fn normalize_override_key(key: &str) -> String {
    key.trim().replace('-', "_").to_ascii_uppercase()
}

/// Field names overridable via `--set` (everything except auto_update).
fn overridable_keys() -> Vec<String> {
    ENV_FIELD_MAP
        .iter()
        .filter(|(s, _)| *s != "AUTO_UPDATE")
        .map(|(_, field)| (*field).to_string())
        .collect()
}

fn normalize_post_commit_push(value: &str) -> String {
    validate_post_commit_push(value).unwrap_or_else(|_| "ask".into())
}

fn validate_post_commit_push(value: &str) -> Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "never" => Ok("never".into()),
        "always" => Ok("always".into()),
        "ask" => Ok("ask".into()),
        _ => anyhow::bail!("Expected one of: ask, always, never"),
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        _ => anyhow::bail!("Expected one of: true, false, 1, 0"),
    }
}

fn parse_usize(value: &str) -> Result<usize> {
    value
        .trim()
        .parse::<usize>()
        .with_context(|| format!("Expected a non-negative integer, got '{value}'"))
}

fn parse_positive_usize(value: &str, name: &str) -> Result<usize> {
    let parsed = parse_usize(value)?;
    if parsed == 0 {
        anyhow::bail!("{name} must be greater than zero");
    }
    Ok(parsed)
}

fn validate_gitmoji_format(value: &str) -> Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "unicode" => Ok("unicode".into()),
        "shortcode" => Ok("shortcode".into()),
        _ => anyhow::bail!("Expected one of: unicode, shortcode"),
    }
}

fn validate_commit_template(value: &str) -> Result<()> {
    if !value.contains("$msg") {
        anyhow::bail!("Commit template must contain '$msg'");
    }
    Ok(())
}

fn validate_globs(globs: &[String]) -> Result<()> {
    for pattern in globs {
        glob::Pattern::new(pattern)
            .with_context(|| format!("Invalid diff exclude glob '{pattern}'"))?;
    }
    Ok(())
}

fn validate_api_headers(raw: &str) -> Result<()> {
    if raw.trim().is_empty() {
        return Ok(());
    }
    if raw.trim_start().starts_with('{') {
        let headers: serde_json::Value =
            serde_json::from_str(raw).context("API headers contain invalid JSON")?;
        let object = headers
            .as_object()
            .context("API headers JSON must be an object")?;
        if object
            .iter()
            .any(|(key, value)| key.trim().is_empty() || !value.is_string())
        {
            anyhow::bail!("API header names must be non-empty and values must be strings");
        }
        return Ok(());
    }
    for pair in raw.split(',') {
        let (key, value) = pair
            .split_once(':')
            .context("API headers must be a JSON object or comma-separated 'Name: Value' pairs")?;
        if key.trim().is_empty()
            || value.trim().is_empty()
            || key.chars().any(char::is_control)
            || value.chars().any(char::is_control)
        {
            anyhow::bail!("API header names and values must be non-empty single-line text");
        }
    }
    Ok(())
}

fn normalize_locale(value: &str) -> String {
    let normalized = value.trim();
    if normalized.is_empty() {
        default_locale()
    } else {
        normalized.replace('_', "-").to_ascii_lowercase()
    }
}

fn validate_locale(locale: &str) -> Result<()> {
    if locale.len() > 35 {
        anyhow::bail!("Locale is too long");
    }
    let valid = locale.split('-').all(|part| {
        !part.is_empty() && part.len() <= 8 && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
    });
    if !valid {
        anyhow::bail!(
            "Invalid locale '{locale}'. Use a language tag such as 'en', 'pt-BR', or 'zh-Hant'."
        );
    }
    Ok(())
}

fn rewrite_dotenv(
    original: &str,
    config: &AppConfig,
    explicit_fields: &HashSet<String>,
) -> Result<String> {
    for suffix in explicit_fields {
        if suffix == "AUTO_UPDATE" || !ENV_FIELD_MAP.iter().any(|(known, _)| known == suffix) {
            anyhow::bail!("Unknown or non-local setting '{suffix}'");
        }
    }

    // Validate the existing file before editing so a malformed multiline value
    // cannot be partially rewritten.
    if !original.is_empty() {
        parse_dotenv_reader(original.as_bytes())?;
    }

    let lines: Vec<&str> = original.split_inclusive('\n').collect();
    let mut rewritten = String::with_capacity(original.len() + 256);
    let mut written = HashSet::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if let Some((key, value_start)) = dotenv_assignment(line) {
            if let Some(suffix) = key.strip_prefix("ACR_") {
                if ENV_FIELD_MAP.iter().any(|(known, _)| *known == suffix) {
                    if explicit_fields.contains(suffix) && suffix != "AUTO_UPDATE" {
                        let value = config.env_value(suffix)?;
                        rewritten.push_str(&format!("ACR_{suffix}={}\n", quote_dotenv(&value)));
                        written.insert(suffix.to_string());
                    }
                    index = consume_assignment(&lines, index, value_start);
                    continue;
                }
            }
        }
        rewritten.push_str(line);
        index += 1;
    }

    for (suffix, _) in ENV_FIELD_MAP {
        if *suffix == "AUTO_UPDATE"
            || !explicit_fields.contains(*suffix)
            || written.contains(*suffix)
        {
            continue;
        }
        if !rewritten.is_empty() && !rewritten.ends_with('\n') {
            rewritten.push('\n');
        }
        let value = config.env_value(suffix)?;
        rewritten.push_str(&format!("ACR_{suffix}={}\n", quote_dotenv(&value)));
    }
    Ok(rewritten)
}

fn dotenv_assignment(line: &str) -> Option<(&str, &str)> {
    let without_newline = line.trim_end_matches(['\r', '\n']);
    let trimmed = without_newline.trim_start();
    let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let (key, value) = trimmed.split_once('=')?;
    let key = key.trim();
    if key.is_empty()
        || !key.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphanumeric() && (index > 0 || !byte.is_ascii_digit())
        })
    {
        return None;
    }
    Some((key, value.trim_start()))
}

fn consume_assignment(lines: &[&str], start: usize, value_start: &str) -> usize {
    let Some(quote) = value_start
        .bytes()
        .next()
        .filter(|byte| *byte == b'\'' || *byte == b'"')
    else {
        return start + 1;
    };
    let mut escaped = false;
    let mut opened = false;
    for (offset, line) in lines[start..].iter().enumerate() {
        let bytes = if offset == 0 {
            value_start.as_bytes()
        } else {
            line.as_bytes()
        };
        for byte in bytes {
            if !opened {
                opened = true;
                continue;
            }
            if quote == b'"' && !escaped && *byte == b'\\' {
                escaped = true;
                continue;
            }
            if !escaped && *byte == quote {
                return start + offset + 1;
            }
            escaped = false;
        }
    }
    lines.len()
}

fn quote_dotenv(value: &str) -> String {
    let json = serde_json::to_string(value).expect("serializing a string cannot fail");
    json.replace('$', "\\$")
}

/// Get description for a field by its env suffix
pub fn field_description(suffix: &str) -> &'static str {
    match suffix {
        "PROVIDER" => "LLM provider (gemini, openai, anthropic, groq, grok, deepseek, openrouter, mistral, together, fireworks, perplexity, lm_studio, ollama, claude_code, codex, opencode, gemini_cli, or custom)",
        "MODEL" => "Model identifier for the selected provider (blank uses a CLI harness default)",
        "API_KEY" => "API key for authenticating with the LLM provider",
        "API_URL" => "Custom API endpoint URL (leave empty to use provider default)",
        "API_HEADERS" => "Additional HTTP headers for API requests (JSON format)",
        "LOCALE" => "Language locale for commit messages (e.g., en, pt-br)",
        "ONE_LINER" => "Generate single-line commit messages when enabled",
        "COMMIT_TEMPLATE" => "Template for commit message ($msg is replaced with generated text)",
        "LLM_SYSTEM_PROMPT" => "System prompt sent to the LLM for context",
        "USE_GITMOJI" => "Prepend gitmoji to commit messages when enabled",
        "GITMOJI_FORMAT" => "Gitmoji style: unicode (🎨) or shortcode (:art:)",
        "REVIEW_COMMIT" => "Review and approve commit message before creating commit",
        "POST_COMMIT_PUSH" => "Push behavior after commit: ask, always, or never",
        "SUPPRESS_TOOL_OUTPUT" => "Hide git command output when enabled",
        "WARN_STAGED_FILES_ENABLED" => "Warn when staged file count exceeds threshold",
        "WARN_STAGED_FILES_THRESHOLD" => "Number of staged files before warning is shown",
        "WARN_LLM_FILES_ENABLED" => "Warn when the count of files sent to the LLM exceeds threshold",
        "WARN_LLM_FILES_THRESHOLD" => "Number of LLM-analyzed files before warning is shown",
        "CONFIRM_NEW_VERSION" => "Ask for confirmation before creating version tags",
        "AUTO_UPDATE" => "Automatically update cgen when new versions are available",
        "FALLBACK_ENABLED" => "Try fallback presets if primary LLM call fails",
        "TRACK_GENERATED_COMMITS" => "Track commits generated by cgen for history view",
        "DIFF_EXCLUDE_GLOBS" => "Comma-separated glob patterns for files to exclude from LLM diff analysis (e.g., *.json,*.lock)",
        "MAX_DIFF_BYTES" => "Maximum filtered diff size sent to an LLM without --allow-large-diff",
        "MAX_OUTPUT_TOKENS" => "Maximum tokens the LLM may generate for one commit message",
        "SENSITIVE_FILE_GLOBS" => "Comma-separated paths that require --allow-sensitive before LLM analysis",
        _ => "",
    }
}

fn parse_dotenv(path: &Path) -> Result<HashMap<String, String>> {
    let iter = dotenvy::from_path_iter(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    iter.collect::<std::result::Result<HashMap<_, _>, _>>()
        .with_context(|| format!("Failed to parse {}", path.display()))
}

fn parse_dotenv_reader(reader: impl std::io::Read) -> Result<HashMap<String, String>> {
    dotenvy::from_read_iter(reader)
        .collect::<std::result::Result<HashMap<_, _>, _>>()
        .context("Failed to parse .env content")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_mask_key_short() {
        assert_eq!(mask_key("abc"), "***");
        assert_eq!(mask_key("12345678"), "********");
    }

    #[test]
    fn test_mask_key_long() {
        assert_eq!(mask_key("abcdefghij"), "abcd...ghij");
        assert_eq!(mask_key("sk-1234567890abcdef"), "sk-1...cdef");
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("exact", 5), "exact");
    }

    #[test]
    fn test_truncate_long() {
        assert_eq!(truncate("hello world", 5), "hello...");
        assert_eq!(truncate("abcdefghij", 3), "abc...");
    }

    #[test]
    fn test_normalize_post_commit_push() {
        assert_eq!(normalize_post_commit_push("never"), "never");
        assert_eq!(normalize_post_commit_push("NEVER"), "never");
        assert_eq!(normalize_post_commit_push("  Never  "), "never");
        assert_eq!(normalize_post_commit_push("always"), "always");
        assert_eq!(normalize_post_commit_push("ALWAYS"), "always");
        assert_eq!(normalize_post_commit_push("ask"), "ask");
        assert_eq!(normalize_post_commit_push("unknown"), "ask");
        assert_eq!(normalize_post_commit_push(""), "ask");
    }

    #[test]
    fn test_parse_usize() {
        assert_eq!(parse_usize("10").unwrap(), 10);
        assert_eq!(parse_usize("  20  ").unwrap(), 20);
        assert!(parse_usize("invalid").is_err());
        assert!(parse_usize("").is_err());
        assert!(parse_usize("-1").is_err());
    }

    #[test]
    fn test_normalize_locale() {
        assert_eq!(normalize_locale("EN"), "en");
        assert_eq!(normalize_locale("  pt-BR  "), "pt-br");
        assert_eq!(normalize_locale("pt_BR"), "pt-br");
        assert_eq!(normalize_locale(""), "en");
        assert_eq!(normalize_locale("   "), "en");
    }

    #[test]
    fn test_unicode_display_helpers_are_char_safe() {
        assert_eq!(mask_key("🔑abcdefgh"), "🔑abc...efgh");
        assert_eq!(truncate("áéíóú", 3), "áéí...");
    }

    #[test]
    fn test_rewrite_dotenv_preserves_unrelated_content() {
        let original = "# app config\nAPP_URL='https://example.test/a b'\nACR_MODEL=old\n\n";
        let config = AppConfig {
            model: "model with spaces".into(),
            ..Default::default()
        };
        let explicit = HashSet::from(["MODEL".to_string()]);
        let rewritten = rewrite_dotenv(original, &config, &explicit).unwrap();
        assert!(rewritten.starts_with(
            "# app config\nAPP_URL='https://example.test/a b'\nACR_MODEL=\"model with spaces\"\n\n"
        ));
    }

    #[test]
    fn test_rewrite_dotenv_removes_inherited_known_key_only() {
        let original = "TOKEN=keep\nACR_API_KEY=remove\nACR_FUTURE=keep-too\n";
        let rewritten = rewrite_dotenv(original, &AppConfig::default(), &HashSet::new()).unwrap();
        assert_eq!(rewritten, "TOKEN=keep\nACR_FUTURE=keep-too\n");
    }

    #[test]
    fn test_dotenv_quoted_value_round_trips() {
        let value = "a value with \"quotes\", $cash, and\nnewlines";
        let content = format!("ACR_API_KEY={}\n", quote_dotenv(value));
        let parsed = parse_dotenv_reader(content.as_bytes()).unwrap();
        assert_eq!(parsed.get("ACR_API_KEY").unwrap(), value);
    }

    #[test]
    fn test_default_functions() {
        assert_eq!(default_provider(), "groq");
        assert_eq!(default_model(), "llama-3.3-70b-versatile");
        assert_eq!(default_locale(), "en");
        assert!(default_true());
        assert_eq!(default_post_commit_push(), "ask");
        assert_eq!(default_commit_template(), "$msg");
        assert_eq!(default_gitmoji_format(), "unicode");
        assert_eq!(default_warn_staged_files_threshold(), 20);
        assert_eq!(default_warn_llm_files_threshold(), 20);
        assert_eq!(default_max_output_tokens(), 512);
    }

    #[test]
    fn test_default_diff_exclude_globs() {
        let globs = default_diff_exclude_globs();
        assert!(globs.contains(&"*.json".to_string()));
        assert!(globs.contains(&"*.lock".to_string()));
        assert!(globs.contains(&"*.png".to_string()));
    }

    #[test]
    fn test_parse_dotenv_basic() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "FOO=bar").unwrap();
        writeln!(file, "BAZ=qux").unwrap();
        let map = parse_dotenv(file.path()).unwrap();
        assert_eq!(map.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(map.get("BAZ"), Some(&"qux".to_string()));
    }

    #[test]
    fn test_parse_dotenv_with_quotes() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "DOUBLE=\"value with spaces\"").unwrap();
        writeln!(file, "SINGLE='another value'").unwrap();
        let map = parse_dotenv(file.path()).unwrap();
        assert_eq!(map.get("DOUBLE"), Some(&"value with spaces".to_string()));
        assert_eq!(map.get("SINGLE"), Some(&"another value".to_string()));
    }

    #[test]
    fn test_parse_dotenv_skips_comments() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "# This is a comment").unwrap();
        writeln!(file, "KEY=value").unwrap();
        writeln!(file, "# Another comment").unwrap();
        let map = parse_dotenv(file.path()).unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn test_parse_dotenv_skips_empty_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file).unwrap();
        writeln!(file, "KEY=value").unwrap();
        writeln!(file, "   ").unwrap();
        let map = parse_dotenv(file.path()).unwrap();
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_parse_dotenv_trims_whitespace() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "  KEY  =  value  ").unwrap();
        let map = parse_dotenv(file.path()).unwrap();
        assert_eq!(map.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn test_field_description_known() {
        assert!(field_description("PROVIDER").contains("lm_studio"));
        assert!(field_description("PROVIDER").contains("ollama"));
        assert!(!field_description("MODEL").is_empty());
        assert!(!field_description("API_KEY").is_empty());
        assert!(!field_description("DIFF_EXCLUDE_GLOBS").is_empty());
    }

    #[test]
    fn test_field_description_unknown() {
        assert_eq!(field_description("UNKNOWN_FIELD"), "");
    }

    #[test]
    fn test_app_config_default() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.provider, "groq");
        assert_eq!(cfg.model, "llama-3.3-70b-versatile");
        assert!(cfg.api_key.is_empty());
        assert!(cfg.one_liner);
        assert!(!cfg.use_gitmoji);
        assert!(cfg.fallback_enabled);
    }

    #[test]
    fn test_app_config_fields_display() {
        let cfg = AppConfig::default();
        let fields = cfg.fields_display();
        assert!(!fields.is_empty());

        // Check some expected fields
        let provider_field = fields.iter().find(|(name, _, _)| *name == "Provider");
        assert!(provider_field.is_some());
        assert_eq!(provider_field.unwrap().2, "groq");
    }

    #[test]
    fn test_app_config_grouped_fields() {
        let cfg = AppConfig::default();
        let groups = cfg.grouped_fields();

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Basic");
        assert_eq!(groups[1].name, "Advanced");

        // Basic group should have direct fields
        assert!(!groups[0].fields.is_empty());

        // Advanced group should have subgroups
        assert!(!groups[1].subgroups.is_empty());
    }

    #[test]
    fn test_app_config_set_field_string() {
        let mut cfg = AppConfig::default();
        cfg.set_field("PROVIDER", "openai").unwrap();
        assert_eq!(cfg.provider, "openai");

        cfg.set_field("MODEL", "gpt-4").unwrap();
        assert_eq!(cfg.model, "gpt-4");
    }

    #[test]
    fn test_app_config_set_field_bool() {
        let mut cfg = AppConfig::default();

        cfg.set_field("ONE_LINER", "false").unwrap();
        assert!(!cfg.one_liner);

        cfg.set_field("ONE_LINER", "true").unwrap();
        assert!(cfg.one_liner);

        cfg.set_field("ONE_LINER", "1").unwrap();
        assert!(cfg.one_liner);

        cfg.set_field("USE_GITMOJI", "TRUE").unwrap();
        assert!(cfg.use_gitmoji);
    }

    #[test]
    fn test_app_config_set_field_usize() {
        let mut cfg = AppConfig::default();
        cfg.set_field("WARN_STAGED_FILES_THRESHOLD", "50").unwrap();
        assert_eq!(cfg.warn_staged_files_threshold, 50);

        assert!(cfg
            .set_field("WARN_STAGED_FILES_THRESHOLD", "invalid")
            .is_err());
        assert_eq!(cfg.warn_staged_files_threshold, 50);
    }

    #[test]
    fn test_app_config_set_field_diff_globs() {
        let mut cfg = AppConfig::default();
        cfg.set_field("DIFF_EXCLUDE_GLOBS", "*.md, *.txt, *.log")
            .unwrap();
        assert_eq!(cfg.diff_exclude_globs, vec!["*.md", "*.txt", "*.log"]);
    }

    #[test]
    fn test_app_config_set_field_post_commit_push() {
        let mut cfg = AppConfig::default();
        cfg.set_field("POST_COMMIT_PUSH", "always").unwrap();
        assert_eq!(cfg.post_commit_push, "always");

        cfg.set_field("POST_COMMIT_PUSH", "NEVER").unwrap();
        assert_eq!(cfg.post_commit_push, "never");

        assert!(cfg.set_field("POST_COMMIT_PUSH", "invalid").is_err());
        assert_eq!(cfg.post_commit_push, "never");
    }

    #[test]
    fn test_app_config_set_field_auto_update() {
        let mut cfg = AppConfig::default();
        assert!(cfg.auto_update.is_none());

        cfg.set_field("AUTO_UPDATE", "true").unwrap();
        assert_eq!(cfg.auto_update, Some(true));

        cfg.set_field("AUTO_UPDATE", "false").unwrap();
        assert_eq!(cfg.auto_update, Some(false));
    }

    #[test]
    fn test_app_config_apply_partial() {
        let mut cfg = AppConfig::default();
        let partial = PartialAppConfig {
            provider: Some("openai".into()),
            model: Some("gpt-4".into()),
            one_liner: Some(false),
            ..Default::default()
        };

        cfg.apply_partial(partial);
        assert_eq!(cfg.provider, "openai");
        assert_eq!(cfg.model, "gpt-4");
        assert!(!cfg.one_liner);
    }

    #[test]
    fn test_app_config_partial_omissions_are_not_merged() {
        let mut cfg = AppConfig {
            api_key: "original-key".into(),
            ..Default::default()
        };
        let partial = PartialAppConfig {
            provider: Some("openai".into()),
            ..Default::default()
        };

        cfg.apply_partial(partial);
        assert_eq!(cfg.provider, "openai");
        assert_eq!(cfg.api_key, "original-key");
        assert!(cfg.review_commit);
    }

    #[test]
    fn test_validate_locale_en() {
        assert!(validate_locale("en").is_ok());
    }

    #[test]
    fn test_validate_locale_invalid() {
        let result = validate_locale("bad--tag");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid locale"));
    }

    #[test]
    fn test_env_field_map_coverage() {
        // Ensure all important fields are in the map
        let suffixes: Vec<&str> = ENV_FIELD_MAP.iter().map(|(s, _)| *s).collect();
        assert!(suffixes.contains(&"PROVIDER"));
        assert!(suffixes.contains(&"MODEL"));
        assert!(suffixes.contains(&"API_KEY"));
        assert!(suffixes.contains(&"DIFF_EXCLUDE_GLOBS"));
        assert!(suffixes.contains(&"FALLBACK_ENABLED"));
    }

    #[test]
    fn test_apply_env_map_all_fields() {
        let mut cfg = AppConfig::default();
        let mut map = HashMap::new();

        map.insert("ACR_PROVIDER".into(), "openai".into());
        map.insert("ACR_MODEL".into(), "gpt-4".into());
        map.insert("ACR_API_KEY".into(), "sk-test".into());
        map.insert("ACR_API_URL".into(), "https://custom.api".into());
        map.insert("ACR_API_HEADERS".into(), r#"{"X-Custom":"value"}"#.into());
        map.insert("ACR_LOCALE".into(), "en".into());
        map.insert("ACR_ONE_LINER".into(), "false".into());
        map.insert("ACR_COMMIT_TEMPLATE".into(), "custom: $msg".into());
        map.insert("ACR_LLM_SYSTEM_PROMPT".into(), "custom prompt".into());
        map.insert("ACR_USE_GITMOJI".into(), "true".into());
        map.insert("ACR_GITMOJI_FORMAT".into(), "shortcode".into());
        map.insert("ACR_REVIEW_COMMIT".into(), "false".into());
        map.insert("ACR_POST_COMMIT_PUSH".into(), "always".into());
        map.insert("ACR_SUPPRESS_TOOL_OUTPUT".into(), "true".into());
        map.insert("ACR_WARN_STAGED_FILES_ENABLED".into(), "false".into());
        map.insert("ACR_WARN_STAGED_FILES_THRESHOLD".into(), "50".into());
        map.insert("ACR_WARN_LLM_FILES_ENABLED".into(), "false".into());
        map.insert("ACR_WARN_LLM_FILES_THRESHOLD".into(), "8".into());
        map.insert("ACR_CONFIRM_NEW_VERSION".into(), "false".into());
        map.insert("ACR_AUTO_UPDATE".into(), "true".into());
        map.insert("ACR_FALLBACK_ENABLED".into(), "false".into());
        map.insert("ACR_TRACK_GENERATED_COMMITS".into(), "false".into());
        map.insert("ACR_DIFF_EXCLUDE_GLOBS".into(), "*.md,*.txt".into());
        map.insert("ACR_MAX_OUTPUT_TOKENS".into(), "256".into());

        cfg.apply_env_map(&map, false).unwrap();

        assert_eq!(cfg.provider, "openai");
        assert_eq!(cfg.model, "gpt-4");
        assert_eq!(cfg.api_key, "sk-test");
        assert_eq!(cfg.api_url, "https://custom.api");
        assert_eq!(cfg.api_headers, r#"{"X-Custom":"value"}"#);
        assert!(!cfg.one_liner);
        assert_eq!(cfg.commit_template, "custom: $msg");
        assert_eq!(cfg.llm_system_prompt, "custom prompt");
        assert!(cfg.use_gitmoji);
        assert_eq!(cfg.gitmoji_format, "shortcode");
        assert!(!cfg.review_commit);
        assert_eq!(cfg.post_commit_push, "always");
        assert!(cfg.suppress_tool_output);
        assert!(!cfg.warn_staged_files_enabled);
        assert_eq!(cfg.warn_staged_files_threshold, 50);
        assert!(!cfg.warn_llm_files_enabled);
        assert_eq!(cfg.warn_llm_files_threshold, 8);
        assert!(!cfg.confirm_new_version);
        assert_eq!(cfg.auto_update, Some(true));
        assert!(!cfg.fallback_enabled);
        assert!(!cfg.track_generated_commits);
        assert_eq!(cfg.diff_exclude_globs, vec!["*.md", "*.txt"]);
        assert_eq!(cfg.max_output_tokens, 256);
    }

    #[test]
    fn test_apply_env_map_auto_update_skipped_for_local() {
        let mut cfg = AppConfig::default();
        let mut map = HashMap::new();
        map.insert("ACR_AUTO_UPDATE".into(), "true".into());

        // from_local = true should skip auto_update
        cfg.apply_env_map(&map, true).unwrap();
        assert!(cfg.auto_update.is_none());

        // from_local = false should apply auto_update
        cfg.apply_env_map(&map, false).unwrap();
        assert_eq!(cfg.auto_update, Some(true));
    }

    #[test]
    fn test_apply_env_map_boolean_variations() {
        let mut cfg = AppConfig::default();
        let mut map = HashMap::new();

        // Test "1" as true
        map.insert("ACR_USE_GITMOJI".into(), "1".into());
        cfg.apply_env_map(&map, false).unwrap();
        assert!(cfg.use_gitmoji);

        // Test "TRUE" (uppercase)
        map.clear();
        map.insert("ACR_REVIEW_COMMIT".into(), "TRUE".into());
        cfg.review_commit = false;
        cfg.apply_env_map(&map, false).unwrap();
        assert!(cfg.review_commit);
    }

    #[test]
    fn test_apply_partial_with_all_fields() {
        let mut cfg = AppConfig::default();
        let partial = PartialAppConfig {
            provider: Some("anthropic".into()),
            model: Some("claude-3".into()),
            api_key: Some("sk-ant".into()),
            api_url: Some("https://api.anthropic.com".into()),
            api_headers: Some(r#"{"x-api-key":"test"}"#.into()),
            locale: Some("es".into()),
            one_liner: Some(false),
            commit_template: Some("feat: $msg".into()),
            llm_system_prompt: Some("custom".into()),
            use_gitmoji: Some(true),
            gitmoji_format: Some("shortcode".into()),
            review_commit: Some(false),
            post_commit_push: Some("never".into()),
            suppress_tool_output: Some(true),
            warn_staged_files_enabled: Some(false),
            warn_staged_files_threshold: Some(100),
            warn_llm_files_enabled: Some(false),
            warn_llm_files_threshold: Some(7),
            confirm_new_version: Some(false),
            auto_update: Some(true),
            fallback_enabled: Some(false),
            track_generated_commits: Some(false),
            diff_exclude_globs: Some(vec!["*.log".into()]),
            max_diff_bytes: Some(123_456),
            max_output_tokens: Some(640),
            sensitive_file_globs: Some(vec!["*.secret".into()]),
        };

        cfg.apply_partial(partial);

        assert_eq!(cfg.provider, "anthropic");
        assert_eq!(cfg.api_url, "https://api.anthropic.com");
        assert_eq!(cfg.api_headers, r#"{"x-api-key":"test"}"#);
        assert_eq!(cfg.auto_update, Some(true));
        assert_eq!(cfg.max_diff_bytes, 123_456);
        assert_eq!(cfg.max_output_tokens, 640);
        assert!(!cfg.warn_llm_files_enabled);
        assert_eq!(cfg.warn_llm_files_threshold, 7);
        assert_eq!(cfg.sensitive_file_globs, vec!["*.secret"]);
    }

    #[test]
    fn test_fields_display_with_custom_values() {
        let cfg = AppConfig {
            api_key: "short".into(), // Short key gets masked differently
            api_url: "https://custom.url".into(),
            api_headers: "X-Custom: value".into(),
            use_gitmoji: true,
            review_commit: false,
            suppress_tool_output: true,
            warn_staged_files_enabled: false,
            confirm_new_version: false,
            auto_update: Some(false),
            fallback_enabled: false,
            track_generated_commits: false,
            diff_exclude_globs: vec![],
            ..Default::default()
        };

        let fields = cfg.fields_display();

        // Find specific fields and check their values
        let api_url = fields.iter().find(|(n, _, _)| *n == "API URL").unwrap();
        assert_eq!(api_url.2, "https://custom.url");

        let api_headers = fields.iter().find(|(n, _, _)| *n == "API Headers").unwrap();
        assert_eq!(api_headers.2, "X-Custom: value");

        let gitmoji = fields.iter().find(|(n, _, _)| *n == "Use Gitmoji").unwrap();
        assert_eq!(gitmoji.2, "enabled");

        let review = fields
            .iter()
            .find(|(n, _, _)| *n == "Review Commit")
            .unwrap();
        assert_eq!(review.2, "disabled");

        let suppress = fields
            .iter()
            .find(|(n, _, _)| *n == "Suppress Tool Output")
            .unwrap();
        assert_eq!(suppress.2, "enabled");

        let warn = fields
            .iter()
            .find(|(n, _, _)| *n == "Warn Staged Files")
            .unwrap();
        assert_eq!(warn.2, "disabled");

        let confirm = fields
            .iter()
            .find(|(n, _, _)| *n == "Confirm New Version")
            .unwrap();
        assert_eq!(confirm.2, "disabled");

        let auto = fields.iter().find(|(n, _, _)| *n == "Auto Update").unwrap();
        assert_eq!(auto.2, "disabled");

        let fallback = fields
            .iter()
            .find(|(n, _, _)| *n == "Fallback Enabled")
            .unwrap();
        assert_eq!(fallback.2, "disabled");

        let track = fields
            .iter()
            .find(|(n, _, _)| *n == "Track Generated Commits")
            .unwrap();
        assert_eq!(track.2, "disabled");

        let globs = fields
            .iter()
            .find(|(n, _, _)| *n == "Diff Exclude Globs")
            .unwrap();
        assert_eq!(globs.2, "(none)");
    }

    #[test]
    fn test_set_field_locale_validation() {
        let mut cfg = AppConfig::default();
        // Valid locale
        let result = cfg.set_field("LOCALE", "en");
        assert!(result.is_ok());
        assert_eq!(cfg.locale, "en");
    }

    #[test]
    fn test_set_field_unknown_is_rejected() {
        let mut cfg = AppConfig::default();
        let original_provider = cfg.provider.clone();
        assert!(cfg.set_field("UNKNOWN_FIELD", "value").is_err());
        assert_eq!(cfg.provider, original_provider);
    }

    #[test]
    fn test_apply_overrides_beats_loaded_value() {
        let mut cfg = AppConfig::default();
        cfg.apply_overrides(&["model=gpt-4o".to_string()]).unwrap();
        assert_eq!(cfg.model, "gpt-4o");
    }

    #[test]
    fn test_apply_overrides_repeated_keys_last_wins() {
        let mut cfg = AppConfig::default();
        cfg.apply_overrides(&["model=a".to_string(), "model=b".to_string()])
            .unwrap();
        assert_eq!(cfg.model, "b");
    }

    #[test]
    fn test_apply_overrides_case_and_hyphen_insensitive() {
        let mut cfg = AppConfig::default();
        cfg.apply_overrides(&["One-Liner=false".to_string()])
            .unwrap();
        assert!(!cfg.one_liner);
    }

    #[test]
    fn test_apply_overrides_rejects_auto_update() {
        let mut cfg = AppConfig::default();
        let err = cfg
            .apply_overrides(&["auto_update=true".to_string()])
            .unwrap_err();
        assert!(err.to_string().contains("auto_update"));
    }

    #[test]
    fn test_apply_overrides_rejects_unknown_key() {
        let mut cfg = AppConfig::default();
        let err = cfg.apply_overrides(&["bogus=1".to_string()]).unwrap_err();
        assert!(err.to_string().contains("Unknown setting"));
    }

    #[test]
    fn test_apply_overrides_rejects_missing_equals() {
        let mut cfg = AppConfig::default();
        let err = cfg.apply_overrides(&["one_liner".to_string()]).unwrap_err();
        assert!(err.to_string().contains("KEY=VALUE"));
    }

    #[test]
    fn test_apply_overrides_value_with_equals_preserved() {
        let mut cfg = AppConfig::default();
        cfg.apply_overrides(&["commit_template=feat: $msg=done".to_string()])
            .unwrap();
        assert_eq!(cfg.commit_template, "feat: $msg=done");
    }

    #[test]
    fn test_apply_overrides_propagates_locale_error() {
        let mut cfg = AppConfig::default();
        let err = cfg
            .apply_overrides(&["locale=bad--tag".to_string()])
            .unwrap_err();
        assert!(format!("{err:#}").contains("Invalid locale"));
    }
}

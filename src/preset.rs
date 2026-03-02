use anyhow::{Context, Result};
use colored::Colorize;
use inquire::{Select, Text};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::AppConfig;
use crate::ui;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmPresetFields {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub api_url: String,
    pub api_headers: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: u32,
    pub name: String,
    #[serde(flatten)]
    pub fields: LlmPresetFields,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FallbackConfig {
    #[serde(default = "crate::config::default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub order: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresetsFile {
    #[serde(default)]
    pub next_id: u32,
    #[serde(default)]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub fallback: FallbackConfig,
}

fn presets_file_path() -> Option<PathBuf> {
    crate::config::global_config_path().map(|p| {
        p.parent()
            .expect("global config path should have a parent")
            .join("presets.toml")
    })
}

pub fn load_presets() -> Result<PresetsFile> {
    let path = match presets_file_path() {
        Some(p) => p,
        None => return Ok(PresetsFile::default()),
    };
    if !path.exists() {
        return Ok(PresetsFile::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let file: PresetsFile =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(file)
}

pub fn save_presets(file: &PresetsFile) -> Result<()> {
    let path = presets_file_path().context("Could not determine presets file path")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let content = toml::to_string_pretty(file).context("Failed to serialize presets")?;
    // Write to temp file then rename for atomicity
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &content)
        .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &path)
        .with_context(|| format!("Failed to rename temp file to {}", path.display()))?;
    Ok(())
}

pub fn fields_from_config(cfg: &AppConfig) -> LlmPresetFields {
    LlmPresetFields {
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
        api_key: cfg.api_key.clone(),
        api_url: cfg.api_url.clone(),
        api_headers: cfg.api_headers.clone(),
    }
}

pub fn apply_preset_to_config(cfg: &mut AppConfig, preset: &Preset) {
    cfg.provider = preset.fields.provider.clone();
    cfg.model = preset.fields.model.clone();
    cfg.api_key = preset.fields.api_key.clone();
    cfg.api_url = preset.fields.api_url.clone();
    cfg.api_headers = preset.fields.api_headers.clone();
}

/// Dedup key: (provider, model, api_key, api_url) — headers excluded
fn dedup_key(fields: &LlmPresetFields) -> (&str, &str, &str, &str) {
    (
        &fields.provider,
        &fields.model,
        &fields.api_key,
        &fields.api_url,
    )
}

pub fn find_duplicate(file: &PresetsFile, fields: &LlmPresetFields) -> Option<u32> {
    let key = dedup_key(fields);
    file.presets
        .iter()
        .find(|p| dedup_key(&p.fields) == key)
        .map(|p| p.id)
}

pub fn create_preset(
    file: &mut PresetsFile,
    name: Option<String>,
    fields: LlmPresetFields,
) -> u32 {
    let id = file.next_id;
    file.next_id += 1;
    let name = name.unwrap_or_else(|| format!("{}/{}", fields.provider, fields.model));
    file.presets.push(Preset {
        id,
        name,
        fields,
    });
    id
}

pub fn delete_preset(file: &mut PresetsFile, id: u32) {
    file.presets.retain(|p| p.id != id);
    file.fallback.order.retain(|&fid| fid != id);
}

pub fn rename_preset(file: &mut PresetsFile, id: u32, new_name: String) {
    if let Some(p) = file.presets.iter_mut().find(|p| p.id == id) {
        p.name = new_name;
    }
}

pub fn duplicate_preset(file: &mut PresetsFile, id: u32) -> Result<u32> {
    let preset = file
        .presets
        .iter()
        .find(|p| p.id == id)
        .context("Preset not found")?
        .clone();
    let new_name = format!("{} (copy)", preset.name);
    let new_id = create_preset(file, Some(new_name), preset.fields);
    Ok(new_id)
}

/// Export presets as standalone TOML. If `!include_keys`, api_key is replaced with "".
pub fn export_presets(file: &PresetsFile, ids: &[u32], include_keys: bool) -> Result<String> {
    let mut export = PresetsFile {
        next_id: 0,
        presets: Vec::new(),
        fallback: FallbackConfig::default(),
    };
    for &id in ids {
        if let Some(p) = file.presets.iter().find(|p| p.id == id) {
            let mut preset = p.clone();
            preset.id = 0; // IDs are reassigned on import
            if !include_keys {
                preset.fields.api_key = String::new();
            }
            export.presets.push(preset);
        }
    }
    toml::to_string_pretty(&export).context("Failed to serialize presets for export")
}

/// Import presets from TOML string. Deduplicates against existing. Returns count imported.
pub fn import_presets(file: &mut PresetsFile, data: &str) -> Result<usize> {
    let imported: PresetsFile =
        toml::from_str(data).context("Failed to parse imported presets data")?;
    let mut count = 0;
    for p in imported.presets {
        if find_duplicate(file, &p.fields).is_some() {
            continue;
        }
        create_preset(file, Some(p.name), p.fields);
        count += 1;
    }
    Ok(count)
}

fn preset_display(p: &Preset) -> String {
    let key_status = if p.fields.api_key.is_empty() {
        "no key"
    } else {
        "key set"
    };
    format!(
        "{} ({}/{}, {})",
        p.name, p.fields.provider, p.fields.model, key_status
    )
}

pub fn interactive_presets() -> Result<()> {
    loop {
        let mut file = load_presets()?;

        if file.presets.is_empty() {
            println!("\n{}", "No presets found.".dimmed());
        } else {
            println!("\n{}", "Presets:".cyan().bold());
            for p in &file.presets {
                println!("  [{}] {}", p.id, preset_display(p));
            }
        }

        let mut choices = vec!["Create new preset"];
        if !file.presets.is_empty() {
            choices.push("Manage existing preset...");
            choices.push("Export presets");
        }
        choices.push("Import presets");
        choices.push("Back");

        let action = match Select::new("Presets:", choices).prompt() {
            Ok(a) => a,
            Err(_) => break,
        };

        match action {
            "Create new preset" => {
                let provider = Text::new("Provider:")
                    .with_default("groq")
                    .prompt()
                    .unwrap_or_default();
                let default_model = crate::provider::default_model_for(&provider);
                let model = Text::new("Model:")
                    .with_default(if default_model.is_empty() {
                        ""
                    } else {
                        default_model
                    })
                    .prompt()
                    .unwrap_or_default();
                let api_key = Text::new("API Key:").prompt().unwrap_or_default();
                let api_url = Text::new("API URL (blank for auto):")
                    .prompt()
                    .unwrap_or_default();
                let api_headers = Text::new("API Headers (blank for auto):")
                    .prompt()
                    .unwrap_or_default();
                let name = Text::new("Preset name (blank for auto):")
                    .prompt()
                    .unwrap_or_default();
                let name = if name.is_empty() { None } else { Some(name) };

                let fields = LlmPresetFields {
                    provider,
                    model,
                    api_key,
                    api_url,
                    api_headers,
                };
                if let Some(dup_id) = find_duplicate(&file, &fields) {
                    println!(
                        "  {} Duplicate of existing preset [{}]",
                        "note:".yellow().bold(),
                        dup_id
                    );
                    continue;
                }
                let id = create_preset(&mut file, name, fields);
                save_presets(&file)?;
                println!("  {} Created preset [{}]", "done!".green().bold(), id);
            }
            "Manage existing preset..." => {
                let options: Vec<String> =
                    file.presets.iter().map(preset_display).collect();
                let Ok(choice) = Select::new("Select preset:", options.clone()).prompt() else {
                    continue;
                };
                let idx = options.iter().position(|o| o == &choice).unwrap();
                let selected_id = file.presets[idx].id;

                let manage_choices = vec!["Rename", "Duplicate", "Delete", "Back"];
                let Ok(manage_action) = Select::new("Action:", manage_choices).prompt() else {
                    continue;
                };

                match manage_action {
                    "Rename" => {
                        if let Ok(new_name) = Text::new("New name:").prompt() {
                            rename_preset(&mut file, selected_id, new_name);
                            save_presets(&file)?;
                            println!("  {}", "Renamed.".green().bold());
                        }
                    }
                    "Duplicate" => {
                        let new_id = duplicate_preset(&mut file, selected_id)?;
                        save_presets(&file)?;
                        println!(
                            "  {} Duplicated as [{}]",
                            "done!".green().bold(),
                            new_id
                        );
                    }
                    "Delete" => {
                        let confirm = ui::confirm("Delete this preset?", false);
                        if confirm {
                            delete_preset(&mut file, selected_id);
                            save_presets(&file)?;
                            println!("  {}", "Deleted.".green().bold());
                        }
                    }
                    _ => {}
                }
            }
            "Export presets" => {
                let include_keys = ui::confirm("Include API keys in export?", false);
                let ids: Vec<u32> = file.presets.iter().map(|p| p.id).collect();
                match export_presets(&file, &ids, include_keys) {
                    Ok(data) => {
                        println!("\n{}", "Exported TOML:".cyan().bold());
                        println!("{data}");
                    }
                    Err(e) => println!("  {} {}", "error:".red().bold(), e),
                }
            }
            "Import presets" => {
                println!("Paste TOML data (end with an empty line):");
                let mut data = String::new();
                loop {
                    let mut line = String::new();
                    if std::io::stdin().read_line(&mut line).is_err() {
                        break;
                    }
                    if line.trim().is_empty() {
                        break;
                    }
                    data.push_str(&line);
                }
                match import_presets(&mut file, &data) {
                    Ok(count) => {
                        save_presets(&file)?;
                        println!(
                            "  {} Imported {} preset(s)",
                            "done!".green().bold(),
                            count
                        );
                    }
                    Err(e) => println!("  {} {}", "error:".red().bold(), e),
                }
            }
            _ => break,
        }
    }
    Ok(())
}

pub fn interactive_fallback_order() -> Result<()> {
    loop {
        let mut file = load_presets()?;

        println!("\n{}", "Fallback Order:".cyan().bold());
        if file.fallback.order.is_empty() {
            println!("  {}", "(empty)".dimmed());
        } else {
            for (i, &id) in file.fallback.order.iter().enumerate() {
                let name = file
                    .presets
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.as_str())
                    .unwrap_or("(missing)");
                println!("  {}. [{}] {}", i + 1, id, name);
            }
        }

        let choices = vec![
            "Add preset",
            "Remove entry",
            "Move up",
            "Move down",
            "Clear all",
            "Back",
        ];

        let action = match Select::new("Configure fallback order:", choices).prompt() {
            Ok(a) => a,
            Err(_) => break,
        };

        match action {
            "Add preset" => {
                let available: Vec<&Preset> = file
                    .presets
                    .iter()
                    .filter(|p| !file.fallback.order.contains(&p.id))
                    .collect();
                if available.is_empty() {
                    println!("  {}", "No presets available to add.".dimmed());
                    continue;
                }
                let options: Vec<String> = available.iter().map(|p| preset_display(p)).collect();
                if let Ok(choice) = Select::new("Select preset to add:", options.clone()).prompt() {
                    let idx = options.iter().position(|o| o == &choice).unwrap();
                    let id = available[idx].id;
                    file.fallback.order.push(id);
                    save_presets(&file)?;
                    println!("  {}", "Added.".green().bold());
                }
            }
            "Remove entry" => {
                if file.fallback.order.is_empty() {
                    continue;
                }
                let options: Vec<String> = file
                    .fallback
                    .order
                    .iter()
                    .map(|&id| {
                        let name = file
                            .presets
                            .iter()
                            .find(|p| p.id == id)
                            .map(|p| p.name.as_str())
                            .unwrap_or("(missing)");
                        format!("[{}] {}", id, name)
                    })
                    .collect();
                if let Ok(choice) =
                    Select::new("Select entry to remove:", options.clone()).prompt()
                {
                    let idx = options.iter().position(|o| o == &choice).unwrap();
                    file.fallback.order.remove(idx);
                    save_presets(&file)?;
                    println!("  {}", "Removed.".green().bold());
                }
            }
            "Move up" => {
                if file.fallback.order.len() < 2 {
                    continue;
                }
                let options: Vec<String> = file
                    .fallback
                    .order
                    .iter()
                    .enumerate()
                    .map(|(i, &id)| {
                        let name = file
                            .presets
                            .iter()
                            .find(|p| p.id == id)
                            .map(|p| p.name.as_str())
                            .unwrap_or("(missing)");
                        format!("{}. [{}] {}", i + 1, id, name)
                    })
                    .collect();
                if let Ok(choice) = Select::new("Move up:", options.clone()).prompt() {
                    let idx = options.iter().position(|o| o == &choice).unwrap();
                    if idx > 0 {
                        file.fallback.order.swap(idx, idx - 1);
                        save_presets(&file)?;
                        println!("  {}", "Moved.".green().bold());
                    }
                }
            }
            "Move down" => {
                if file.fallback.order.len() < 2 {
                    continue;
                }
                let options: Vec<String> = file
                    .fallback
                    .order
                    .iter()
                    .enumerate()
                    .map(|(i, &id)| {
                        let name = file
                            .presets
                            .iter()
                            .find(|p| p.id == id)
                            .map(|p| p.name.as_str())
                            .unwrap_or("(missing)");
                        format!("{}. [{}] {}", i + 1, id, name)
                    })
                    .collect();
                if let Ok(choice) = Select::new("Move down:", options.clone()).prompt() {
                    let idx = options.iter().position(|o| o == &choice).unwrap();
                    if idx < file.fallback.order.len() - 1 {
                        file.fallback.order.swap(idx, idx + 1);
                        save_presets(&file)?;
                        println!("  {}", "Moved.".green().bold());
                    }
                }
            }
            "Clear all" => {
                let confirm = ui::confirm("Clear entire fallback order?", false);
                if confirm {
                    file.fallback.order.clear();
                    save_presets(&file)?;
                    println!("  {}", "Cleared.".green().bold());
                }
            }
            _ => break,
        }
    }
    Ok(())
}

/// Select and load a preset into the config. Returns (preset_id, snapshot) if loaded.
pub fn select_and_load_preset(cfg: &mut AppConfig) -> Result<Option<(u32, LlmPresetFields)>> {
    let file = load_presets()?;
    if file.presets.is_empty() {
        println!("  {}", "No presets found.".dimmed());
        return Ok(None);
    }
    let options: Vec<String> = file.presets.iter().map(preset_display).collect();
    match Select::new("Select preset to load:", options.clone()).prompt() {
        Ok(choice) => {
            let idx = options.iter().position(|o| o == &choice).unwrap();
            let preset = &file.presets[idx];
            let snapshot = preset.fields.clone();
            apply_preset_to_config(cfg, preset);
            println!(
                "  {} Loaded preset: {}",
                "done!".green().bold(),
                preset.name
            );
            Ok(Some((preset.id, snapshot)))
        }
        Err(_) => Ok(None),
    }
}

/// Save current config LLM fields as a new preset.
pub fn save_current_as_preset(cfg: &AppConfig) -> Result<()> {
    let fields = fields_from_config(cfg);
    let mut file = load_presets()?;

    if let Some(dup_id) = find_duplicate(&file, &fields) {
        println!(
            "  {} Already saved as preset [{}]",
            "note:".yellow().bold(),
            dup_id
        );
        return Ok(());
    }

    let name = Text::new("Preset name (blank for auto):")
        .prompt()
        .unwrap_or_default();
    let name = if name.is_empty() { None } else { Some(name) };

    let id = create_preset(&mut file, name, fields);
    save_presets(&file)?;
    println!("  {} Created preset [{}]", "done!".green().bold(), id);
    Ok(())
}

/// Check if current config fields differ from the loaded preset snapshot.
pub fn preset_is_modified(cfg: &AppConfig, snapshot: &LlmPresetFields) -> bool {
    let current = fields_from_config(cfg);
    current != *snapshot
}

/// Prompt user to update the loaded preset with current config fields.
pub fn prompt_update_preset(cfg: &AppConfig, preset_id: u32) -> Result<()> {
    let should_update = ui::confirm("Update the loaded preset too?", false);
    if !should_update {
        return Ok(());
    }
    let mut file = load_presets()?;
    if let Some(p) = file.presets.iter_mut().find(|p| p.id == preset_id) {
        p.fields = fields_from_config(cfg);
        save_presets(&file)?;
        println!("  {} Preset updated.", "done!".green().bold());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fields() -> LlmPresetFields {
        LlmPresetFields {
            provider: "groq".into(),
            model: "llama-3.3-70b-versatile".into(),
            api_key: "test-key".into(),
            api_url: String::new(),
            api_headers: String::new(),
        }
    }

    #[test]
    fn test_create_preset_auto_name() {
        let mut file = PresetsFile::default();
        let fields = sample_fields();
        let id = create_preset(&mut file, None, fields);
        assert_eq!(id, 0);
        assert_eq!(file.presets.len(), 1);
        assert_eq!(file.presets[0].name, "groq/llama-3.3-70b-versatile");
        assert_eq!(file.next_id, 1);
    }

    #[test]
    fn test_create_preset_custom_name() {
        let mut file = PresetsFile::default();
        let id = create_preset(&mut file, Some("My Preset".into()), sample_fields());
        assert_eq!(file.presets[0].name, "My Preset");
        assert_eq!(id, 0);
    }

    #[test]
    fn test_find_duplicate() {
        let mut file = PresetsFile::default();
        let fields = sample_fields();
        create_preset(&mut file, None, fields.clone());
        assert_eq!(find_duplicate(&file, &fields), Some(0));

        let different = LlmPresetFields {
            provider: "openai".into(),
            ..fields
        };
        assert_eq!(find_duplicate(&file, &different), None);
    }

    #[test]
    fn test_find_duplicate_ignores_headers() {
        let mut file = PresetsFile::default();
        let fields = sample_fields();
        create_preset(&mut file, None, fields.clone());

        let with_headers = LlmPresetFields {
            api_headers: "X-Custom: value".into(),
            ..fields
        };
        assert_eq!(find_duplicate(&file, &with_headers), Some(0));
    }

    #[test]
    fn test_delete_preset_removes_from_fallback() {
        let mut file = PresetsFile::default();
        let id = create_preset(&mut file, None, sample_fields());
        file.fallback.order.push(id);

        delete_preset(&mut file, id);
        assert!(file.presets.is_empty());
        assert!(file.fallback.order.is_empty());
    }

    #[test]
    fn test_rename_preset() {
        let mut file = PresetsFile::default();
        let id = create_preset(&mut file, None, sample_fields());
        rename_preset(&mut file, id, "New Name".into());
        assert_eq!(file.presets[0].name, "New Name");
    }

    #[test]
    fn test_duplicate_preset() {
        let mut file = PresetsFile::default();
        let id = create_preset(&mut file, Some("Original".into()), sample_fields());
        let new_id = duplicate_preset(&mut file, id).unwrap();
        assert_eq!(file.presets.len(), 2);
        assert_eq!(file.presets[1].name, "Original (copy)");
        assert_ne!(id, new_id);
    }

    #[test]
    fn test_export_without_keys() {
        let mut file = PresetsFile::default();
        let id = create_preset(&mut file, None, sample_fields());
        let exported = export_presets(&file, &[id], false).unwrap();
        assert!(exported.contains("api_key = \"\""));
    }

    #[test]
    fn test_export_with_keys() {
        let mut file = PresetsFile::default();
        let id = create_preset(&mut file, None, sample_fields());
        let exported = export_presets(&file, &[id], true).unwrap();
        assert!(exported.contains("test-key"));
    }

    #[test]
    fn test_import_deduplicates() {
        let mut file = PresetsFile::default();
        create_preset(&mut file, None, sample_fields());

        let import_data = toml::to_string_pretty(&file).unwrap();
        let count = import_presets(&mut file, &import_data).unwrap();
        assert_eq!(count, 0);
        assert_eq!(file.presets.len(), 1);
    }

    #[test]
    fn test_import_new_preset() {
        let mut file = PresetsFile::default();

        let mut import_file = PresetsFile::default();
        create_preset(&mut import_file, Some("Imported".into()), sample_fields());
        let import_data = toml::to_string_pretty(&import_file).unwrap();

        let count = import_presets(&mut file, &import_data).unwrap();
        assert_eq!(count, 1);
        assert_eq!(file.presets.len(), 1);
    }

    #[test]
    fn test_fields_from_config() {
        let cfg = AppConfig::default();
        let fields = fields_from_config(&cfg);
        assert_eq!(fields.provider, "groq");
        assert_eq!(fields.model, "llama-3.3-70b-versatile");
    }

    #[test]
    fn test_apply_preset_to_config() {
        let mut cfg = AppConfig::default();
        let preset = Preset {
            id: 0,
            name: "test".into(),
            fields: LlmPresetFields {
                provider: "openai".into(),
                model: "gpt-4o".into(),
                api_key: "sk-test".into(),
                api_url: String::new(),
                api_headers: String::new(),
            },
        };
        apply_preset_to_config(&mut cfg, &preset);
        assert_eq!(cfg.provider, "openai");
        assert_eq!(cfg.model, "gpt-4o");
        assert_eq!(cfg.api_key, "sk-test");
    }

    #[test]
    fn test_preset_is_modified() {
        let cfg = AppConfig::default();
        let snapshot = fields_from_config(&cfg);
        assert!(!preset_is_modified(&cfg, &snapshot));

        let mut modified_cfg = cfg.clone();
        modified_cfg.provider = "openai".into();
        assert!(preset_is_modified(&modified_cfg, &snapshot));
    }

    #[test]
    fn test_incrementing_ids() {
        let mut file = PresetsFile::default();
        let id1 = create_preset(&mut file, None, sample_fields());
        let fields2 = LlmPresetFields {
            provider: "openai".into(),
            ..sample_fields()
        };
        let id2 = create_preset(&mut file, None, fields2);
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(file.next_id, 2);
    }

    #[test]
    fn test_preset_display_with_key() {
        let preset = Preset {
            id: 1,
            name: "My Preset".into(),
            fields: LlmPresetFields {
                provider: "groq".into(),
                model: "llama".into(),
                api_key: "sk-test".into(),
                api_url: String::new(),
                api_headers: String::new(),
            },
        };
        let display = preset_display(&preset);
        assert!(display.contains("My Preset"));
        assert!(display.contains("groq"));
        assert!(display.contains("llama"));
        assert!(display.contains("key set"));
    }

    #[test]
    fn test_preset_display_no_key() {
        let preset = Preset {
            id: 1,
            name: "Empty Key".into(),
            fields: LlmPresetFields {
                provider: "openai".into(),
                model: "gpt-4".into(),
                api_key: String::new(),
                api_url: String::new(),
                api_headers: String::new(),
            },
        };
        let display = preset_display(&preset);
        assert!(display.contains("no key"));
    }

    #[test]
    fn test_dedup_key_extracts_correct_fields() {
        let fields = LlmPresetFields {
            provider: "groq".into(),
            model: "llama".into(),
            api_key: "key123".into(),
            api_url: "https://api.example.com".into(),
            api_headers: "X-Custom: value".into(),
        };
        let key = dedup_key(&fields);
        assert_eq!(key.0, "groq");
        assert_eq!(key.1, "llama");
        assert_eq!(key.2, "key123");
        assert_eq!(key.3, "https://api.example.com");
    }

    #[test]
    fn test_fallback_config_serde() {
        let config = FallbackConfig {
            enabled: true,
            order: vec![1, 2, 3],
        };
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: FallbackConfig = toml::from_str(&toml_str).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.order, vec![1, 2, 3]);
    }

    #[test]
    fn test_presets_file_serde_full() {
        let mut file = PresetsFile::default();
        create_preset(&mut file, Some("Test".into()), sample_fields());
        file.fallback.order.push(0);

        let toml_str = toml::to_string_pretty(&file).unwrap();
        let parsed: PresetsFile = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.presets.len(), 1);
        assert_eq!(parsed.presets[0].name, "Test");
        assert_eq!(parsed.fallback.order, vec![0]);
    }

    #[test]
    fn test_duplicate_preset_preserves_fields() {
        let mut file = PresetsFile::default();
        let fields = LlmPresetFields {
            provider: "anthropic".into(),
            model: "claude".into(),
            api_key: "sk-ant".into(),
            api_url: "https://api.anthropic.com".into(),
            api_headers: "x-api-key: test".into(),
        };
        let id = create_preset(&mut file, Some("Original".into()), fields);
        let dup_id = duplicate_preset(&mut file, id).unwrap();

        let dup = file.presets.iter().find(|p| p.id == dup_id).unwrap();
        assert_eq!(dup.fields.provider, "anthropic");
        assert_eq!(dup.fields.model, "claude");
        assert_eq!(dup.fields.api_key, "sk-ant");
    }

    #[test]
    fn test_rename_nonexistent_preset_does_nothing() {
        let mut file = PresetsFile::default();
        create_preset(&mut file, Some("Original".into()), sample_fields());
        rename_preset(&mut file, 999, "New Name".into());
        // Should not panic, original unchanged
        assert_eq!(file.presets[0].name, "Original");
    }

    #[test]
    fn test_export_presets_multiple() {
        let mut file = PresetsFile::default();
        let id1 = create_preset(&mut file, Some("First".into()), sample_fields());
        let fields2 = LlmPresetFields {
            provider: "openai".into(),
            ..sample_fields()
        };
        let id2 = create_preset(&mut file, Some("Second".into()), fields2);

        let exported = export_presets(&file, &[id1, id2], true).unwrap();
        assert!(exported.contains("First"));
        assert!(exported.contains("Second"));
    }

    #[test]
    fn test_import_presets_multiple() {
        let mut import_file = PresetsFile::default();
        create_preset(&mut import_file, Some("Import1".into()), sample_fields());
        let fields2 = LlmPresetFields {
            provider: "openai".into(),
            ..sample_fields()
        };
        create_preset(&mut import_file, Some("Import2".into()), fields2);

        let import_data = toml::to_string_pretty(&import_file).unwrap();

        let mut file = PresetsFile::default();
        let count = import_presets(&mut file, &import_data).unwrap();

        assert_eq!(count, 2);
        assert_eq!(file.presets.len(), 2);
    }

    #[test]
    fn test_delete_preset_nonexistent() {
        let mut file = PresetsFile::default();
        create_preset(&mut file, None, sample_fields());
        delete_preset(&mut file, 999); // Non-existent ID
        assert_eq!(file.presets.len(), 1); // Original still there
    }

    #[test]
    fn test_llm_preset_fields_clone() {
        let fields = sample_fields();
        let cloned = fields.clone();
        assert_eq!(fields.provider, cloned.provider);
        assert_eq!(fields.model, cloned.model);
    }
}

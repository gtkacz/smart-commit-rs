mod common;

use auto_commit_rs::preset::{
    create_preset, delete_preset, duplicate_preset, export_presets, find_duplicate,
    import_presets, load_presets, rename_preset, save_presets, FallbackConfig, LlmPresetFields,
    Preset, PresetsFile,
};
use common::EnvGuard;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

fn setup_presets_env() -> (TempDir, EnvGuard) {
    let cfg_dir = TempDir::new().expect("tempdir");
    let cgen_dir = cfg_dir.path().join("cgen");
    fs::create_dir_all(&cgen_dir).expect("create cgen dir");
    let env = EnvGuard::set(&[(
        "ACR_CONFIG_HOME",
        cfg_dir.path().to_string_lossy().as_ref(),
    )]);
    (cfg_dir, env)
}

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
#[serial]
fn load_presets_returns_default_when_no_file() {
    let (_cfg_dir, _env) = setup_presets_env();
    let file = load_presets().expect("load_presets should succeed");
    assert!(file.presets.is_empty());
    assert_eq!(file.next_id, 0);
}

#[test]
#[serial]
fn save_and_load_presets_roundtrip() {
    let (cfg_dir, _env) = setup_presets_env();

    let mut file = PresetsFile::default();
    create_preset(&mut file, Some("Test Preset".into()), sample_fields());

    save_presets(&file).expect("save_presets should succeed");

    // Verify file was created
    let presets_path = cfg_dir.path().join("cgen").join("presets.toml");
    assert!(presets_path.exists(), "presets.toml should be created");

    // Load and verify
    let loaded = load_presets().expect("load_presets should succeed");
    assert_eq!(loaded.presets.len(), 1);
    assert_eq!(loaded.presets[0].name, "Test Preset");
    assert_eq!(loaded.next_id, 1);
}

#[test]
#[serial]
fn save_presets_atomic_write() {
    let (cfg_dir, _env) = setup_presets_env();

    let mut file = PresetsFile::default();
    create_preset(&mut file, None, sample_fields());
    save_presets(&file).expect("first save");

    // Modify and save again
    create_preset(
        &mut file,
        Some("Second".into()),
        LlmPresetFields {
            provider: "openai".into(),
            ..sample_fields()
        },
    );
    save_presets(&file).expect("second save");

    // Verify no temp files left behind
    let cgen_dir = cfg_dir.path().join("cgen");
    let tmp_files: Vec<_> = fs::read_dir(&cgen_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "tmp"))
        .collect();

    assert!(
        tmp_files.is_empty(),
        "no temp files should remain after save"
    );
}

#[test]
fn presets_file_default() {
    let file = PresetsFile::default();
    assert_eq!(file.next_id, 0);
    assert!(file.presets.is_empty());
    assert!(file.fallback.order.is_empty());
}

#[test]
fn fallback_config_default() {
    let config = FallbackConfig::default();
    assert!(config.order.is_empty());
    // enabled defaults to true via serde default
}

#[test]
fn preset_struct_fields() {
    let preset = Preset {
        id: 42,
        name: "My Preset".into(),
        fields: sample_fields(),
    };

    assert_eq!(preset.id, 42);
    assert_eq!(preset.name, "My Preset");
    assert_eq!(preset.fields.provider, "groq");
}

#[test]
fn export_presets_empty_ids() {
    let file = PresetsFile::default();
    let exported = export_presets(&file, &[], true).unwrap();
    let parsed: PresetsFile = toml::from_str(&exported).unwrap();
    assert!(parsed.presets.is_empty());
}

#[test]
fn export_presets_nonexistent_id() {
    let mut file = PresetsFile::default();
    create_preset(&mut file, None, sample_fields());

    // Export with non-existent ID
    let exported = export_presets(&file, &[999], true).unwrap();
    let parsed: PresetsFile = toml::from_str(&exported).unwrap();
    assert!(parsed.presets.is_empty());
}

#[test]
fn import_presets_empty_data() {
    let mut file = PresetsFile::default();
    let data = toml::to_string(&PresetsFile::default()).unwrap();
    let count = import_presets(&mut file, &data).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn import_presets_invalid_toml_fails() {
    let mut file = PresetsFile::default();
    let result = import_presets(&mut file, "this is not valid { toml");
    assert!(result.is_err());
}

#[test]
fn delete_preset_nonexistent_id() {
    let mut file = PresetsFile::default();
    create_preset(&mut file, None, sample_fields());
    let initial_len = file.presets.len();

    delete_preset(&mut file, 999);

    assert_eq!(file.presets.len(), initial_len, "should not delete anything");
}

#[test]
fn rename_preset_nonexistent_id() {
    let mut file = PresetsFile::default();
    create_preset(&mut file, Some("Original".into()), sample_fields());

    rename_preset(&mut file, 999, "New Name".into());

    assert_eq!(
        file.presets[0].name, "Original",
        "should not rename anything"
    );
}

#[test]
fn duplicate_preset_nonexistent_fails() {
    let mut file = PresetsFile::default();
    let result = duplicate_preset(&mut file, 999);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn find_duplicate_empty_file() {
    let file = PresetsFile::default();
    assert_eq!(find_duplicate(&file, &sample_fields()), None);
}

#[test]
fn llm_preset_fields_equality() {
    let f1 = sample_fields();
    let f2 = sample_fields();
    assert_eq!(f1, f2);

    let f3 = LlmPresetFields {
        provider: "openai".into(),
        ..sample_fields()
    };
    assert_ne!(f1, f3);
}

#[test]
fn preset_serde_with_flatten() {
    let preset = Preset {
        id: 1,
        name: "Test".into(),
        fields: sample_fields(),
    };

    let toml_str = toml::to_string(&preset).unwrap();
    // Verify flatten works - fields should be at top level
    assert!(toml_str.contains("provider ="));
    assert!(toml_str.contains("model ="));

    let parsed: Preset = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.id, preset.id);
    assert_eq!(parsed.fields.provider, preset.fields.provider);
}

#[test]
fn fallback_order_manipulation() {
    let mut file = PresetsFile::default();
    let id1 = create_preset(&mut file, Some("First".into()), sample_fields());
    let id2 = create_preset(
        &mut file,
        Some("Second".into()),
        LlmPresetFields {
            provider: "openai".into(),
            ..sample_fields()
        },
    );

    file.fallback.order.push(id1);
    file.fallback.order.push(id2);

    assert_eq!(file.fallback.order, vec![id1, id2]);

    // Delete first preset should remove from fallback order
    delete_preset(&mut file, id1);
    assert_eq!(file.fallback.order, vec![id2]);
}

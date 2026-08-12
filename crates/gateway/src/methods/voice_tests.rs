//! Tests for the voice provider method handlers.
//!
//! Extracted from `voice.rs` to keep that module under the workspace file-size
//! limit. Declared via `#[cfg(test)] #[path = "voice_tests.rs"] mod tests;` in
//! `voice.rs`, so `super::*` resolves to the voice module's items.

use std::path::PathBuf;

use {super::*, secrecy::Secret};

fn test_voice_provider(id: VoiceProviderId) -> VoiceProviderInfo {
    let meta = id.meta();
    VoiceProviderInfo {
        id,
        name: String::new(),
        provider_type: String::new(),
        category: String::new(),
        description: meta.description.to_string(),
        available: false,
        enabled: false,
        preferred: false,
        key_source: None,
        key_placeholder: meta.key_placeholder.map(str::to_string),
        key_url: meta.key_url.map(str::to_string),
        key_url_label: meta.key_url_label.map(str::to_string),
        hint: meta.hint.map(str::to_string),
        binary_path: None,
        status_message: None,
        capabilities: serde_json::json!({}),
        settings: serde_json::json!({}),
        settings_summary: None,
    }
}

#[test]
fn parse_voice_provider_list_aliases() {
    assert_eq!(
        VoiceProviderId::parse_tts_list_id("openai"),
        Some(VoiceProviderId::OpenaiTts)
    );
    assert_eq!(
        VoiceProviderId::parse_tts_list_id("google-tts"),
        Some(VoiceProviderId::GoogleTts)
    );
    assert_eq!(
        VoiceProviderId::parse_stt_list_id("elevenlabs"),
        Some(VoiceProviderId::ElevenlabsStt)
    );
    assert_eq!(
        VoiceProviderId::parse_stt_list_id("sherpa-onnx"),
        Some(VoiceProviderId::SherpaOnnx)
    );
}

#[test]
fn filter_listed_voice_providers_keeps_all_when_list_is_empty() {
    let filtered = filter_listed_voice_providers(
        vec![
            test_voice_provider(VoiceProviderId::OpenaiTts),
            test_voice_provider(VoiceProviderId::GoogleTts),
        ],
        &[],
        VoiceProviderId::parse_tts_list_id,
    );
    assert_eq!(filtered.len(), 2);
}

#[test]
fn filter_listed_voice_providers_filters_tts_ids() {
    let filtered = filter_listed_voice_providers(
        vec![
            test_voice_provider(VoiceProviderId::OpenaiTts),
            test_voice_provider(VoiceProviderId::GoogleTts),
            test_voice_provider(VoiceProviderId::Piper),
        ],
        &["openai".to_string(), "piper".to_string()],
        VoiceProviderId::parse_tts_list_id,
    );
    let ids: Vec<_> = filtered.into_iter().map(|p| p.id).collect();
    assert_eq!(ids, vec![
        VoiceProviderId::OpenaiTts,
        VoiceProviderId::Piper
    ]);
}

#[tokio::test]
async fn detect_voice_providers_marks_selected_stt_provider_when_some() {
    let mut config = moltis_config::MoltisConfig::default();
    config.voice.stt.enabled = true;
    config.voice.stt.provider = Some(VoiceSttProvider::Whisper);
    config.voice.stt.whisper.api_key = Some(Secret::new("test-whisper-key".to_string()));

    let detected = detect_voice_providers(&config).await;
    let Some(stt) = detected["stt"].as_array() else {
        panic!("stt list missing");
    };
    let Some(whisper) = stt.iter().find(|provider| provider["id"] == "whisper") else {
        panic!("whisper provider missing");
    };

    assert_eq!(whisper["enabled"], serde_json::json!(true));
}

#[tokio::test]
async fn detect_voice_providers_does_not_mark_stt_preferred_when_none() {
    let mut config = moltis_config::MoltisConfig::default();
    config.voice.stt.enabled = true;
    config.voice.stt.provider = None;
    config.voice.stt.whisper.api_key = Some(Secret::new("test-whisper-key".to_string()));

    let detected = detect_voice_providers(&config).await;
    let Some(stt) = detected["stt"].as_array() else {
        panic!("stt list missing");
    };
    let preferred_count = stt
        .iter()
        .filter(|provider| provider["preferred"].as_bool() == Some(true))
        .count();

    assert_eq!(preferred_count, 0);
}

#[test]
fn apply_voice_provider_settings_stores_base_urls() {
    let mut config = moltis_config::MoltisConfig::default();

    apply_voice_provider_settings(
        &mut config,
        "openai",
        &serde_json::json!({
            "baseUrl": "http://127.0.0.1:8003/v1",
        }),
    );
    apply_voice_provider_settings(
        &mut config,
        "whisper",
        &serde_json::json!({
            "baseUrl": "http://127.0.0.1:8001/v1",
            "model": "gpt-4o-mini-transcribe",
        }),
    );

    assert_eq!(
        config.voice.tts.openai.base_url.as_deref(),
        Some("http://127.0.0.1:8003/v1")
    );
    assert_eq!(
        config.voice.stt.whisper.base_url.as_deref(),
        Some("http://127.0.0.1:8001/v1")
    );
    assert_eq!(config.voice.stt.provider, Some(VoiceSttProvider::Whisper));
    assert!(config.voice.stt.enabled);
    assert_eq!(
        config.voice.stt.whisper.model.as_deref(),
        Some("gpt-4o-mini-transcribe")
    );
}

#[test]
fn apply_voice_provider_settings_clears_base_urls_when_requested() {
    let mut config = moltis_config::MoltisConfig::default();
    config.voice.tts.openai.base_url = Some("http://127.0.0.1:8003/v1".to_string());
    config.voice.stt.whisper.base_url = Some("http://127.0.0.1:8001/v1".to_string());
    config.voice.stt.whisper.model = Some("gpt-4o-mini-transcribe".to_string());

    apply_voice_provider_settings(
        &mut config,
        "openai",
        &serde_json::json!({
            "baseUrl": "",
        }),
    );
    apply_voice_provider_settings(
        &mut config,
        "whisper",
        &serde_json::json!({
            "baseUrl": "",
            "model": "",
        }),
    );

    assert_eq!(config.voice.tts.openai.base_url, None);
    assert_eq!(config.voice.stt.whisper.base_url, None);
    assert_eq!(config.voice.stt.whisper.model, None);
}

#[tokio::test]
async fn detect_voice_providers_marks_whisper_available_when_base_url_configured() {
    let mut config = moltis_config::MoltisConfig::default();
    config.voice.stt.whisper.base_url = Some("http://127.0.0.1:8001/v1".to_string());

    let detected = detect_voice_providers(&config).await;
    let Some(stt) = detected["stt"].as_array() else {
        panic!("stt list missing");
    };
    let Some(whisper) = stt.iter().find(|provider| provider["id"] == "whisper") else {
        panic!("whisper provider missing");
    };

    assert_eq!(whisper["available"], serde_json::json!(true));
    assert_eq!(whisper["keySource"], serde_json::json!("config"));
    assert_eq!(
        whisper["settings"]["baseUrl"],
        serde_json::json!("http://127.0.0.1:8001/v1")
    );
    assert_eq!(
        whisper["capabilities"]["realtimeModelChoices"],
        serde_json::json!([
            "gpt-realtime-2",
            "gpt-realtime-translate",
            "gpt-realtime-whisper"
        ])
    );
}

// ---------------------------------------------------------------------------
// `resolve_local_binary` + configured-`binary_path` detection regressions.
//
// Before the fix, `detect_voice_providers` ran `which <name>` and ignored the
// user-configured `binary_path` for whisper-cli, piper, and sherpa-onnx. That
// made `available=false` even when a working binary was configured elsewhere
// (e.g. a renamed build), which hid the WebUI mic button via `voice-input.ts`.
// ---------------------------------------------------------------------------

#[test]
fn resolve_local_binary_returns_none_when_binary_absent() {
    assert_eq!(
        resolve_local_binary("definitely-not-in-path-xyz123", None),
        None
    );
}

#[test]
fn resolve_local_binary_prefers_configured_path_over_path_lookup() {
    // The test binary itself is guaranteed to exist and be a regular file.
    let Ok(exe) = std::env::current_exe() else {
        panic!("current_exe unavailable");
    };
    let Some(resolved) = resolve_local_binary(
        // A name guaranteed not to be on PATH, so the only way to resolve is
        // via the configured path.
        "definitely-not-in-path-xyz123",
        exe.to_str(),
    ) else {
        panic!("configured binary path was not honored");
    };
    assert_eq!(resolved, exe.to_string_lossy());
}

#[test]
fn resolve_local_binary_expands_tilde_in_configured_path() {
    let Some(home) = std::env::var_os("HOME") else {
        // Mirrors the guarded tilde test in moltis-voice's cli_utils: without a
        // HOME dir the expansion can't be exercised.
        return;
    };
    let home_path = PathBuf::from(home);
    let fake_name = format!(".moltis-resolve-tilde-test-{}", std::process::id());
    let abs_path = home_path.join(&fake_name);
    let tilde_path = format!("~/{fake_name}");

    if let Err(err) = std::fs::write(&abs_path, b"#!/bin/sh\n") {
        panic!("failed to write fake binary at {abs_path:?}: {err}");
    }
    let result = resolve_local_binary("not-in-path-tilde-test", Some(&tilde_path));
    let _ = std::fs::remove_file(&abs_path);

    let Some(resolved) = result else {
        panic!("tilde path was not expanded/resolved");
    };
    assert_eq!(resolved, abs_path.to_string_lossy());
}

#[tokio::test]
async fn detect_voice_providers_marks_whisper_cli_available_with_configured_binary_path() {
    // Regression: with binary_path set to a real binary and a model configured,
    // whisper-cli must report available=true (previously detection ran
    // `which whisper-cpp`/`which whisper` and ignored binary_path).
    let Ok(exe) = std::env::current_exe() else {
        panic!("current_exe unavailable");
    };
    let mut config = moltis_config::MoltisConfig::default();
    config.voice.stt.enabled = true;
    config.voice.stt.whisper_cli.binary_path = Some(exe.to_string_lossy().into_owned());
    config.voice.stt.whisper_cli.model_path = Some("/tmp/moltis-test-model.bin".to_string());

    let detected = detect_voice_providers(&config).await;
    let Some(stt) = detected["stt"].as_array() else {
        panic!("stt list missing");
    };
    let Some(whisper_cli) = stt.iter().find(|provider| provider["id"] == "whisper-cli") else {
        panic!("whisper-cli provider missing");
    };

    assert_eq!(whisper_cli["available"], serde_json::json!(true));
    assert_eq!(
        whisper_cli["binaryPath"],
        serde_json::json!(exe.to_string_lossy().as_ref())
    );
}

#[tokio::test]
async fn detect_voice_providers_marks_piper_available_with_configured_binary_path() {
    let Ok(exe) = std::env::current_exe() else {
        panic!("current_exe unavailable");
    };
    let mut config = moltis_config::MoltisConfig::default();
    config.voice.tts.enabled = true;
    config.voice.tts.piper.binary_path = Some(exe.to_string_lossy().into_owned());
    config.voice.tts.piper.model_path = Some("/tmp/moltis-test-piper.onnx".to_string());

    let detected = detect_voice_providers(&config).await;
    let Some(tts) = detected["tts"].as_array() else {
        panic!("tts list missing");
    };
    let Some(piper) = tts.iter().find(|provider| provider["id"] == "piper") else {
        panic!("piper provider missing");
    };

    assert_eq!(piper["available"], serde_json::json!(true));
    assert_eq!(
        piper["binaryPath"],
        serde_json::json!(exe.to_string_lossy().as_ref())
    );
}

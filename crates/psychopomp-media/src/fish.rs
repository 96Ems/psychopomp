//! Fish Audio Text to Speech, requested as `fish-say` (and so
//! `scripts/narrate.ts`) requests it. Fish returns no timestamps; Whisper
//! times its words.
use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::spec::SpeechSpec;

pub(crate) fn speech(agent: &ureq::Agent, key: &str, spec: &SpeechSpec) -> Result<Vec<u8>> {
    let line = &spec.lines[0];
    let body = json!({
        "text": line.text,
        "reference_id": line.voice,
        "format": "wav",
        "sample_rate": 44_100,
        "latency": "normal",
        "prosody": {
            "speed": spec.settings.speed.unwrap_or(1.0),
            "volume": 0,
            "normalize_loudness": true,
        },
    });
    let model = spec.model.as_deref().context("Fish Audio needs a model")?;
    let mut response = agent
        .post("https://api.fish.audio/v1/tts")
        .header("authorization", &format!("Bearer {key}"))
        .header("content-type", "application/json")
        .header("model", model)
        .send(serde_json::to_vec(&body)?)
        .context("Fish Audio request failed")?;
    let status = response.status().as_u16();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(256 << 20)
        .read_to_vec()
        .context("read the Fish Audio response")?;
    if !(200..300).contains(&status) {
        let hint = match status {
            401 => "check the API key",
            402 => "check API credit or use the free model",
            403 | 404 => "check that this key can use the voice",
            429 => "rate limited; retry later",
            _ => "check Fish Audio's status and the request",
        };
        bail!("Fish Audio HTTP {status}: {hint}");
    }
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("Fish Audio did not return WAV audio");
    }
    Ok(bytes)
}

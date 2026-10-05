//! ElevenLabs: Text to Speech with character timestamps, Text to Dialogue
//! (with timestamps, or plain when Whisper times it), and Sound Effects.
//! Authenticated with `xi-api-key`; the key and audio are never printed.
use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    spec::{PROVIDER_ALIGN, Route, SoundSpec, SpeechSpec},
    words::Alignment,
};

const API: &str = "https://api.elevenlabs.io";

pub(crate) struct Reply {
    pub audio: Vec<u8>,
    pub request: Option<String>,
    /// What ElevenLabs billed, from its `character-cost` header.
    pub credits: Option<u64>,
    pub alignment: Option<Alignment>,
}

#[derive(Deserialize)]
struct Timed {
    audio_base64: String,
    alignment: Option<Alignment>,
}

/// One speech or dialogue request for `spec`, stitched after `previous`.
pub(crate) fn speech(
    agent: &ureq::Agent,
    key: &str,
    spec: &SpeechSpec,
    previous: &[String],
) -> Result<Reply> {
    let timed = spec.align == PROVIDER_ALIGN;
    let settings = &spec.settings;
    let mut body = Map::new();
    body.insert("model_id".into(), json!(spec.model));
    let url = match spec.route {
        Route::Speech => {
            let line = &spec.lines[0];
            body.insert("text".into(), json!(line.text));
            let mut voice = Map::new();
            if let Some(stability) = settings.stability {
                voice.insert("stability".into(), json!(stability));
            }
            if let Some(similarity) = settings.similarity {
                voice.insert("similarity_boost".into(), json!(similarity));
            }
            if !voice.is_empty() {
                body.insert("voice_settings".into(), Value::Object(voice));
            }
            format!(
                "{API}/v1/text-to-speech/{}/with-timestamps?output_format={}",
                line.voice, spec.format
            )
        }
        Route::Dialogue => {
            let inputs = spec
                .lines
                .iter()
                .map(|line| json!({ "text": line.text, "voice_id": line.voice }))
                .collect::<Vec<_>>();
            body.insert("inputs".into(), Value::Array(inputs));
            let mut dialogue = Map::new();
            if let Some(stability) = settings.stability {
                dialogue.insert("stability".into(), json!(stability));
            }
            if let Some(similarity) = settings.similarity {
                dialogue.insert("similarity".into(), json!(similarity));
            }
            if !dialogue.is_empty() {
                body.insert("settings".into(), Value::Object(dialogue));
            }
            let route = if timed {
                "text-to-dialogue/with-timestamps"
            } else {
                "text-to-dialogue"
            };
            format!("{API}/v1/{route}?output_format={}", spec.format)
        }
    };
    if let Some(seed) = settings.seed {
        body.insert("seed".into(), json!(seed));
    }
    if let Some(language) = &settings.language {
        body.insert("language_code".into(), json!(language));
    }
    if !previous.is_empty() {
        body.insert("previous_request_ids".into(), json!(previous));
    }
    body.insert("use_pvc_as_ivc".into(), json!(settings.ivc));
    let timed = timed || spec.route == Route::Speech;
    post(agent, key, &url, &Value::Object(body), timed)
}

/// One sound effect.
pub(crate) fn sound(agent: &ureq::Agent, key: &str, spec: &SoundSpec) -> Result<Reply> {
    let mut body = Map::new();
    body.insert("text".into(), json!(spec.prompt));
    body.insert("model_id".into(), json!(spec.model));
    body.insert(
        "duration_seconds".into(),
        json!(spec.duration_nanos as f64 / 1e9),
    );
    if let Some(influence) = spec.influence {
        body.insert("prompt_influence".into(), json!(influence));
    }
    if spec.looping {
        body.insert("loop".into(), json!(true));
    }
    let url = format!("{API}/v1/sound-generation?output_format={}", spec.format);
    post(agent, key, &url, &Value::Object(body), false)
}

fn post(agent: &ureq::Agent, key: &str, url: &str, body: &Value, timed: bool) -> Result<Reply> {
    let mut response = agent
        .post(url)
        .header("xi-api-key", key)
        .header("content-type", "application/json")
        .send(serde_json::to_vec(body)?)
        .context("ElevenLabs request failed")?;
    let status = response.status().as_u16();
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let request = header("request-id");
    let credits = header("character-cost").and_then(|cost| cost.parse().ok());
    let content_type = header("content-type").unwrap_or_default();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(256 << 20)
        .read_to_vec()
        .context("read the ElevenLabs response")?;
    if !(200..300).contains(&status) {
        bail!("ElevenLabs HTTP {status}: {}", detail(&bytes));
    }
    if !timed {
        if !content_type.starts_with("audio/") {
            bail!("ElevenLabs returned {content_type}, not audio");
        }
        return Ok(Reply {
            audio: bytes,
            request,
            credits,
            alignment: None,
        });
    }
    let reply: Timed =
        serde_json::from_slice(&bytes).context("parse the ElevenLabs timed response")?;
    let audio = base64::engine::general_purpose::STANDARD
        .decode(reply.audio_base64)
        .context("decode ElevenLabs audio")?;
    Ok(Reply {
        audio,
        request,
        credits,
        alignment: reply.alignment,
    })
}

/// The human part of an error body: `detail.message`, `detail.status`, or a
/// validation list. Bodies never contain the key.
fn detail(bytes: &[u8]) -> String {
    let Ok(body) = serde_json::from_slice::<Value>(bytes) else {
        return String::from_utf8_lossy(bytes).chars().take(300).collect();
    };
    let detail = &body["detail"];
    detail["message"]
        .as_str()
        .or_else(|| detail["status"].as_str())
        .map(str::to_owned)
        .or_else(|| detail.as_str().map(str::to_owned))
        .unwrap_or_else(|| detail.to_string().chars().take(300).collect())
}

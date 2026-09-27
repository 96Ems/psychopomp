use crate::{
    plan_runtime::Scene,
    render::{HeadlessRenderer, Theme},
};
use kinograph::playback::{PlaybackCommand, PlaybackSpeed};
use std::time::Duration;
use wasm_bindgen::prelude::*;

fn error(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}
fn clock(ms: f64) -> Result<Duration, JsValue> {
    if !ms.is_finite() || !(0. ..=1e12).contains(&ms) {
        return Err(error("invalid clock"));
    }
    Ok(Duration::from_secs_f64(ms / 1000.))
}

#[wasm_bindgen]
pub struct GridCanvas {
    renderer: HeadlessRenderer,
    scene: Scene,
}

#[wasm_bindgen]
impl GridCanvas {
    pub async fn create(
        canvas: web_sys::HtmlCanvasElement,
        plan: &str,
        glyphs: &[u8],
    ) -> Result<GridCanvas, JsValue> {
        console_error_panic_hook::set_once();
        let mut renderer = HeadlessRenderer::new(canvas, glyphs).await.map_err(error)?;
        let scene =
            Scene::new(serde_json::from_str(plan).map_err(error)?, &mut renderer).map_err(error)?;
        Ok(Self { renderer, scene })
    }

    pub fn load(&mut self, plan: &str) -> Result<(), JsValue> {
        self.scene = Scene::new(
            serde_json::from_str(plan).map_err(error)?,
            &mut self.renderer,
        )
        .map_err(error)?;
        Ok(())
    }

    /// Benchmark fence, not a per-frame wait in normal interactive playback.
    pub async fn completed(&self) -> Result<(), JsValue> {
        let (send, receive) = futures_channel::oneshot::channel();
        self.renderer.queue.on_submitted_work_done(move || {
            let _ = send.send(());
        });
        receive.await.map_err(error)
    }

    pub fn draw(&mut self, now_ms: f64) -> Result<String, JsValue> {
        let sample = self.scene.playback.sample(clock(now_ms)?);
        let time = sample.at_nanos as f64 / 1e9;
        self.scene
            .render(&mut self.renderer, time, &self.scene.playback.timeline())
            .map_err(error)?;
        Ok(serde_json::json!({ "scene": self.scene.plan.id, "step": sample.step_index, "phase": format!("{:?}", sample.phase), "time": time, "speed": self.scene.playback.speed().label(), "reduced": self.scene.playback.reduced_motion(), "title": self.scene.plan.presentation_steps[sample.step_index].title }).to_string())
    }

    /// Explicit arbitrary-time authored sampling for parity checks, not Previous.
    pub fn authored(&mut self, seconds: f64) -> Result<(), JsValue> {
        clock(seconds * 1000.)?;
        self.scene
            .render(&mut self.renderer, seconds, &self.scene.timeline)
            .map_err(error)?;
        Ok(())
    }

    pub fn action(&mut self, name: &str, now_ms: f64) -> Result<(), JsValue> {
        let now = clock(now_ms)?;
        let p = &mut self.scene.playback;
        match name {
            "next" => {
                p.command(PlaybackCommand::Next, now);
            }
            "previous" => {
                p.command(PlaybackCommand::Previous, now);
            }
            "replay" => {
                p.command(PlaybackCommand::Replay, now);
            }
            "pause" => {
                p.command(PlaybackCommand::TogglePause, now);
            }
            "freeze" => {
                p.pause(now);
            }
            "first" => {
                p.command(PlaybackCommand::First, now);
            }
            "last" => {
                p.command(PlaybackCommand::Last, now);
            }
            "slower" => {
                p.set_speed(p.speed().cycle(false), now);
            }
            "faster" => {
                p.set_speed(p.speed().cycle(true), now);
            }
            "frame-next" => {
                p.step_frame(false, now);
            }
            "frame-previous" => {
                p.step_frame(true, now);
            }
            "replay-paused" => {
                p.command(PlaybackCommand::Replay, now);
                p.pause(now);
            }
            "reduce" => {
                p.set_reduced_motion(!p.reduced_motion(), now);
            }
            speed if speed.starts_with("speed:") => {
                p.set_speed(PlaybackSpeed::parse(&speed[6..]).map_err(error)?, now);
            }
            _ => return Err(error("unknown action")),
        }
        Ok(())
    }

    pub fn theme(&mut self, name: &str) -> Result<(), JsValue> {
        self.renderer.set_theme(Theme::parse(name).map_err(error)?);
        Ok(())
    }
}

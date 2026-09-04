use std::{
    sync::{
        Arc,
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Instant,
};

use anyhow::{Context, Result};
use kinograph::{playback::PlaybackSample, timeline::Timeline};
use winit::event_loop::EventLoopProxy;

use super::super::{PreparedPlan, VisualSampleKey};
use crate::render::HeadlessRenderer;

pub(super) enum RenderEvent {
    Frame {
        slide_index: usize,
        sample: PlaybackSample,
        pixels: Arc<Vec<u8>>,
        requested_at: Instant,
        render_time: std::time::Duration,
    },
    Failed(String),
}

pub(super) struct RenderWorker {
    requests: Option<SyncSender<RenderRequest>>,
    thread: Option<JoinHandle<()>>,
}

struct RenderRequest {
    slide_index: usize,
    sample: PlaybackSample,
    timeline: Arc<Timeline>,
    requested_at: Instant,
}

impl RenderWorker {
    pub(super) fn spawn(
        slides: Vec<PreparedPlan>,
        mut renderer: HeadlessRenderer,
        proxy: EventLoopProxy<RenderEvent>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<RenderRequest>(1);
        let thread = thread::Builder::new()
            .name("kinograph-render".into())
            .spawn(move || {
                let result = (|| -> Result<()> {
                    let mut cached: Option<(usize, VisualSampleKey, Arc<Vec<u8>>)> = None;
                    let mut count = 0_u32;
                    let mut total = std::time::Duration::ZERO;
                    while let Ok(RenderRequest {
                        slide_index,
                        sample,
                        timeline,
                        requested_at,
                    }) = receiver.recv()
                    {
                        let render_started = Instant::now();
                        let prepared =
                            slides.get(slide_index).context("unknown requested slide")?;
                        renderer.set_file_name(prepared.file_name());
                        let time = sample.at_nanos as f64 / 1_000_000_000.0;
                        let key = prepared.visual_sample_key_using(time, &timeline)?;
                        let pixels = if let Some((previous_slide, previous, pixels)) = &cached
                            && *previous_slide == slide_index
                            && *previous == key
                        {
                            pixels.clone()
                        } else {
                            let start = Instant::now();
                            let pixels = Arc::new(prepared.render_sample_using(
                                &mut renderer,
                                time,
                                &timeline,
                            )?);
                            total += start.elapsed();
                            count += 1;
                            cached = Some((slide_index, key, pixels.clone()));
                            pixels
                        };
                        if proxy
                            .send_event(RenderEvent::Frame {
                                slide_index,
                                sample,
                                pixels,
                                requested_at,
                                render_time: render_started.elapsed(),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    if count > 0 {
                        eprintln!(
                            "Native preview: {count} fresh samples, {:.1} ms/sample mean",
                            total.as_secs_f64() * 1000.0 / f64::from(count)
                        );
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    let _ = proxy.send_event(RenderEvent::Failed(format!("{error:#}")));
                }
            })
            .context("start render worker")?;
        Ok(Self {
            requests: Some(sender),
            thread: Some(thread),
        })
    }

    pub(super) fn request(
        &self,
        slide_index: usize,
        sample: PlaybackSample,
        timeline: Arc<Timeline>,
    ) -> Result<()> {
        self.requests
            .as_ref()
            .context("render worker has stopped")?
            .try_send(RenderRequest {
                slide_index,
                sample,
                timeline,
                requested_at: Instant::now(),
            })
            .context("send render request")
    }
}

impl Drop for RenderWorker {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::new_renderer;
    use super::*;
    use std::path::Path;

    #[test]
    #[ignore = "requires a headless GPU and fonts; measures the actual live sample path"]
    fn live_sampling_cost_and_out_of_order_pixels() {
        let plan = kinograph_effect_succeed_slides::build_plan().unwrap();
        let mut renderer = pollster::block_on(new_renderer(&plan.id)).unwrap();
        let prepared = PreparedPlan::prepare(plan, Path::new("."), &mut renderer).unwrap();
        renderer.set_file_name(prepared.file_name());
        for preview in [false, true] {
            renderer.set_interactive_preview(preview);
            let initial = prepared.render_sample(&mut renderer, 0.0).unwrap();
            let final_frame = prepared.render_sample(&mut renderer, 12.5).unwrap();
            let mut elapsed = Vec::new();
            for index in 0..24 {
                let time = 1.0 + f64::from(index) / 60.0;
                let start = Instant::now();
                prepared.render_sample(&mut renderer, time).unwrap();
                elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            assert_eq!(initial, prepared.render_sample(&mut renderer, 0.0).unwrap());
            assert_eq!(
                final_frame,
                prepared.render_sample(&mut renderer, 12.5).unwrap()
            );
            elapsed.sort_by(f64::total_cmp);
            eprintln!(
                "1080p preview={preview}: median {:.2}ms, p95 {:.2}ms",
                elapsed[12], elapsed[22]
            );
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; proves live retarget rendering, not just scalar values"]
    fn interrupted_reversal_preserves_the_rendered_frame() {
        use kinograph::playback::{Playback, PlaybackCommand};
        use std::time::Duration;
        let plan = kinograph_effect_succeed_slides::build_plan().unwrap();
        let mut renderer = pollster::block_on(new_renderer(&plan.id)).unwrap();
        let prepared = PreparedPlan::prepare(plan.clone(), Path::new("."), &mut renderer).unwrap();
        renderer.set_file_name(prepared.file_name());
        renderer.set_interactive_preview(true);
        let mut playback = Playback::new(&plan, &prepared.timeline, false).unwrap();
        let initial = prepared
            .render_sample_using(&mut renderer, 0., &playback.timeline())
            .unwrap();
        playback.command(PlaybackCommand::Next, Duration::ZERO);
        let before = prepared
            .render_sample_using(&mut renderer, 0.15, &playback.timeline())
            .unwrap();
        assert_ne!(initial, before);
        playback.command(PlaybackCommand::Previous, Duration::from_millis(150));
        let after = prepared
            .render_sample_using(&mut renderer, 0.15, &playback.timeline())
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            initial,
            prepared
                .render_sample_using(&mut renderer, 2., &playback.timeline())
                .unwrap()
        );
        assert_eq!(
            after,
            prepared
                .render_sample_using(&mut renderer, 0.15, &playback.timeline())
                .unwrap()
        );
    }
}

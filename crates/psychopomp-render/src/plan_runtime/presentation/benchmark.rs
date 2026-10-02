//! One warmup second, then nine identical one-second interruption rounds.
//! These are native frame-submission measurements, not claimed display scanout times.
use std::time::{Duration, Instant};

use super::{Options, gpu};
use psychopomp::playback::PlaybackCommand;
use serde::Serialize;

const WARMUP: f64 = 1.0;
const ROUNDS: usize = 9;
const ACTION_INTERVAL: Duration = Duration::from_millis(250);

pub(super) struct Benchmark {
    start: Instant,
    action: u32,
    previous: Option<Instant>,
    frames: Vec<Frame>,
    size: [u32; 2],
    resized: bool,
    gpu_completion: bool,
    full_quality: bool,
    reduced_motion: bool,
}

#[derive(Serialize)]
struct Frame {
    slide_index: usize,
    round: usize,
    interval_ms: f64,
    prepare_ms: f64,
    present_ms: f64,
    render_ms: f64,
    age_ms: f64,
    acquire_ms: f64,
    completed_work_ms: Option<f64>,
}

impl Benchmark {
    pub(super) fn new(options: &Options) -> Self {
        Self {
            start: Instant::now(),
            action: 0,
            previous: None,
            frames: Vec::new(),
            size: [0; 2],
            resized: false,
            gpu_completion: options.benchmark_gpu,
            full_quality: options.full_quality,
            reduced_motion: options.reduced_motion,
        }
    }
    pub(super) fn finished(&self) -> bool {
        self.start.elapsed().as_secs_f64() >= WARMUP + ROUNDS as f64
    }
    pub(super) fn measuring(&self) -> bool {
        self.start.elapsed().as_secs_f64() >= WARMUP
    }
    pub(super) fn slide_index(&self, count: usize) -> usize {
        ((self.start.elapsed().as_secs_f64() - WARMUP).max(0.0) / 2.0) as usize % count
    }
    pub(super) fn deadline(&self) -> Instant {
        self.start + ACTION_INTERVAL * self.action
    }
    pub(super) fn command(&mut self) -> Option<PlaybackCommand> {
        if Instant::now() < self.deadline() {
            return None;
        }
        let command = [
            PlaybackCommand::Next,
            PlaybackCommand::Last,
            PlaybackCommand::Previous,
            PlaybackCommand::First,
        ][self.action as usize % 4];
        self.action += 1;
        Some(command)
    }
    pub(super) fn frame(
        &mut self,
        now: Instant,
        slide_index: usize,
        size: [u32; 2],
        timing: gpu::Timing,
        render: Duration,
        age: Duration,
    ) {
        let elapsed = now.duration_since(self.start).as_secs_f64();
        let previous = self.previous.replace(now);
        if self.size != [0; 2] && self.size != size {
            self.resized = true;
        }
        self.size = size;
        if !(WARMUP..WARMUP + ROUNDS as f64).contains(&elapsed) {
            return;
        }
        if let Some(previous) = previous {
            self.frames.push(Frame {
                slide_index,
                round: (elapsed - WARMUP) as usize,
                interval_ms: now.duration_since(previous).as_secs_f64() * 1000.,
                prepare_ms: timing.prepare.as_secs_f64() * 1000.,
                present_ms: timing.present.as_secs_f64() * 1000.,
                render_ms: render.as_secs_f64() * 1000.,
                age_ms: age.as_secs_f64() * 1000.,
                acquire_ms: timing.acquire.as_secs_f64() * 1000.,
                completed_work_ms: timing
                    .completed_work
                    .map(|work| (render + work).as_secs_f64() * 1000.),
            });
        }
    }
    pub(super) fn report(
        &self,
        interval: Duration,
        refresh_millihertz: Option<u32>,
    ) -> Result<(), &'static str> {
        if self.resized {
            return Err("benchmark window changed physical size; rerun without resizing");
        }
        if (0..ROUNDS).any(|round| !self.frames.iter().any(|frame| frame.round == round)) {
            return Err(
                "benchmark did not present frames in every round; keep the benchmark window visible for all ten seconds",
            );
        }
        let rounds = (0..ROUNDS)
            .map(|round| {
                let frames = self
                    .frames
                    .iter()
                    .filter(|frame| frame.round == round)
                    .collect::<Vec<_>>();
                serde_json::json!({
                    "round": round + 1,
                    "frames": frames.len(),
                    "intervalP50Ms": percentile(frames.iter().map(|frame| frame.interval_ms), 0.5),
                    "intervalP95Ms": percentile(frames.iter().map(|frame| frame.interval_ms), 0.95),
                    "prepareP50Ms": percentile(frames.iter().map(|frame| frame.prepare_ms), 0.5),
                    "presentP50Ms": percentile(frames.iter().map(|frame| frame.present_ms), 0.5),
                    "acquireP50Ms": percentile(frames.iter().map(|frame| frame.acquire_ms), 0.5),
                    "renderP50Ms": percentile(frames.iter().map(|frame| frame.render_ms), 0.5),
                    "ageP95Ms": percentile(frames.iter().map(|frame| frame.age_ms), 0.95),
                    "completedWorkP50Ms": self.gpu_completion.then(|| percentile(frames.iter().filter_map(|frame| frame.completed_work_ms), 0.5)),
                    "completedWorkP95Ms": self.gpu_completion.then(|| percentile(frames.iter().filter_map(|frame| frame.completed_work_ms), 0.95)),
                })
            })
            .collect::<Vec<_>>();
        let p95 = percentile(
            rounds
                .iter()
                .filter_map(|round| round["intervalP95Ms"].as_f64()),
            0.5,
        );
        eprintln!("METRIC native_frame_p95_ms={p95:.3}");
        let work_p95 = self.gpu_completion.then(|| {
            percentile(
                rounds
                    .iter()
                    .filter_map(|round| round["completedWorkP95Ms"].as_f64()),
                0.5,
            )
        });
        if let Some(work) = work_p95 {
            eprintln!("METRIC completed_frame_work_p95_ms={work:.3}");
        }
        println!(
            "{}",
            serde_json::json!({"physicalSize": self.size, "samplingHz": 1.0 / interval.as_secs_f64(), "displayHz": refresh_millihertz.map(|rate| f64::from(rate) / 1000.), "fullQuality": self.full_quality, "reducedMotion": self.reduced_motion, "gpuCompletionMeasured": self.gpu_completion, "warmupSeconds": WARMUP, "medianRoundP95Ms": p95, "medianRoundCompletedWorkP95Ms": work_p95, "rounds": rounds, "frames": self.frames})
        );
        Ok(())
    }
}

fn percentile(values: impl Iterator<Item = f64>, fraction: f64) -> f64 {
    let mut values = values.collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return 0.0;
    }
    values[((values.len() - 1) as f64 * fraction).round() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_work_excludes_drawable_wait_and_does_not_double_count_upload() {
        let mut benchmark = Benchmark::new(&Options::default());
        let now = benchmark.start + Duration::from_millis(1500);
        benchmark.previous = Some(now - Duration::from_millis(17));
        benchmark.frame(
            now,
            0,
            [2560, 1440],
            gpu::Timing {
                prepare: Duration::from_millis(1),
                present: Duration::from_millis(14),
                acquire: Duration::from_millis(12),
                completed_work: Some(Duration::from_millis(3)),
            },
            Duration::from_millis(2),
            Duration::from_millis(18),
        );
        assert_eq!(benchmark.frames.len(), 1);
        let frame = &benchmark.frames[0];
        assert_eq!(frame.round, 0);
        assert_eq!(frame.completed_work_ms, Some(5.0));
        assert_eq!(frame.acquire_ms, 12.0);
    }

    #[test]
    fn incomplete_or_resized_runs_are_not_reported_as_fast() {
        let mut benchmark = Benchmark::new(&Options::default());
        let interval = Duration::from_nanos(16_666_666);
        assert!(benchmark.report(interval, Some(60_000)).is_err());
        benchmark.resized = true;
        assert!(
            benchmark
                .report(interval, Some(60_000))
                .unwrap_err()
                .contains("size")
        );
    }
}

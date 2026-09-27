//! Bounded boxes and attached straight wires; all choreography stays in the plan.
use crate::render::{DiagramGlyphs, HeadlessRenderer};
use anyhow::{Context, Result, bail};
use kinograph::{
    component_prototype::{DIAGRAM, DiagramPlan},
    plan::ScenePlan,
    playback::StartDelay,
};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

pub(super) struct PreparedDiagram {
    id: String,
    plan: DiagramPlan,
    glyphs: DiagramGlyphs,
    delays: Vec<(String, StartDelay)>,
}

fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
fn text(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 180 && !s.contains(['\n', '\r'])
}

pub(super) fn validate_recipe(
    actor_id: &str,
    p: &DiagramPlan,
    channels: &[kinograph::plan::ContinuousChannelPlan],
) -> Result<()> {
    if p.bounds.iter().any(|v| !v.is_finite())
        || p.bounds[0] < 0.
        || p.bounds[1] < 0.
        || !(800. ..=1920.).contains(&p.bounds[2])
        || !(500. ..=1080.).contains(&p.bounds[3])
        || p.bounds[0] + p.bounds[2] > 1920.
        || p.bounds[1] + p.bounds[3] > 1080.
        || !text(&p.title)
        || p.captions.is_empty()
        || p.captions.len() > 12
        || p.captions.iter().any(|s| !text(s))
        || p.nodes.is_empty()
        || p.nodes.len() > 16
        || p.links.len() > 32
    {
        bail!(
            "diagram needs bounded canvas layout, 1..16 nodes, <=32 links and 1..12 single-line captions"
        );
    }
    let mut ids = HashSet::new();
    let mut properties: HashSet<String> = ["caption".into()].into();
    for i in 0..p.captions.len() {
        for property in ["active", "seen"] {
            properties.insert(format!("indicator.{i}.{property}"));
        }
    }
    for n in &p.nodes {
        if !identifier(&n.id)
            || !ids.insert(&n.id)
            || !text(&n.label)
            || n.alternate_label.as_deref().is_some_and(|s| !text(s))
            || n.center.iter().chain(&n.size).any(|v| !v.is_finite())
            || n.center.iter().any(|v| v.abs() > 3840.)
            || !(80. ..=800.).contains(&n.size[0])
            || !(40. ..=240.).contains(&n.size[1])
        {
            bail!("invalid or duplicate diagram node");
        }
        for property in [
            "x",
            "y",
            "width",
            "height",
            "opacity",
            "shell",
            "ink",
            "scale",
            "blur",
            "label",
            "emphasis",
            "glow",
            "depth",
            "lift",
            "width-reveal",
        ] {
            properties.insert(format!("node.{}.{property}", n.id));
        }
    }
    let mut links = HashSet::new();
    for link in &p.links {
        if !identifier(&link.id) || !links.insert(&link.id) {
            bail!("invalid or duplicate diagram link");
        }
        for anchor in [&link.from, &link.to] {
            if !ids.contains(&anchor.node)
                || !anchor.offset.is_finite()
                || anchor.offset.abs() > 400.
            {
                bail!("diagram link references an invalid node/port");
            }
        }
        for property in ["draw", "opacity", "emphasis", "from-offset", "to-offset"] {
            properties.insert(format!("link.{}.{property}", link.id));
        }
    }
    for c in channels.iter().filter(|c| c.actor_id == actor_id) {
        if !properties.contains(&c.property) {
            bail!("unknown diagram property '{}'", c.property);
        }
    }
    let mut delays = HashSet::new();
    for d in &p.delays {
        if !delays.insert(&d.property)
            || d.millis > 600
            || !d.from.is_finite()
            || !d.to.is_finite()
            || !channels
                .iter()
                .any(|c| c.actor_id == actor_id && c.property == d.property)
        {
            bail!(
                "diagram start delay needs a unique declared channel and <=600ms finite resting-pose wait"
            );
        }
    }
    Ok(())
}

impl PreparedDiagram {
    // The isolated browser stages this concrete entrypoint; native preflight
    // already owns the parsed input and calls from_recipe directly.
    #[allow(dead_code)]
    pub(super) fn prepare(
        scene: &ScenePlan,
        renderer: &mut HeadlessRenderer,
    ) -> Result<Option<Self>> {
        scene
            .actors
            .iter()
            .find(|a| a.recipe == DIAGRAM)
            .map(|a| {
                let plan: DiagramPlan = serde_json::from_value(a.data.clone())?;
                validate_recipe(&a.id, &plan, &scene.continuous_channels)?;
                Self::from_recipe(a.id.clone(), plan, &scene.continuous_channels, renderer)
            })
            .transpose()
    }

    pub(super) fn from_recipe(
        id: String,
        plan: DiagramPlan,
        channels: &[kinograph::plan::ContinuousChannelPlan],
        renderer: &mut HeadlessRenderer,
    ) -> Result<Self> {
        let glyphs = renderer.prepare_diagram(&plan)?;
        let delays = plan
            .delays
            .iter()
            .map(|d| {
                let c = channels
                    .iter()
                    .find(|c| c.actor_id == id && c.property == d.property)
                    .context("declared diagram delay")?;
                Ok((
                    c.id.clone(),
                    StartDelay {
                        from: d.from,
                        to: d.to,
                        delay: Duration::from_millis(d.millis),
                    },
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            id,
            plan,
            glyphs,
            delays,
        })
    }
    pub(super) fn delays(&self, target: &mut HashMap<String, StartDelay>) {
        target.extend(self.delays.iter().cloned());
    }
    pub(super) fn render(
        &self,
        renderer: &mut HeadlessRenderer,
        value: impl Fn(&str, &str, f32) -> f32,
    ) -> Result<Vec<u8>> {
        renderer.render_diagram(&self.plan, &self.glyphs, |p, d| value(&self.id, p, d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_runtime::{PreparedPlan, new_renderer, validate_renderer_plan};
    use kinograph::{playback::PlaybackCommand, timeline::PropertyId};
    use std::path::Path;

    #[test]
    fn isometric_formation_keeps_room_for_the_growing_volume() {
        let scene = kinograph_opencode_architecture::build_deck()
            .unwrap()
            .slides
            .remove(1)
            .plan;
        let prepared = crate::plan_runtime::CompiledPlan::compile(scene, Path::new(".")).unwrap();
        let value = |kind: &str, index, property, time, default| {
            prepared.property_value(
                &prepared.timeline,
                "diagram",
                &format!("node.{kind}-{index}.{property}"),
                time,
                default,
            )
        };
        for (start, incoming) in [(3., 1), (6., 2)] {
            for millis in 1..800 {
                let at = start + millis as f64 / 1000.;
                for kind in ["client", "server"] {
                    if value(kind, incoming, "opacity", at, 1.) < 0.01 {
                        continue;
                    }
                    let width = |index| {
                        value(kind, index, "width", at, 300.)
                            * value(kind, index, "width-reveal", at, 1.)
                    };
                    let gap = value(kind, incoming, "x", at, 0.)
                        - value(kind, incoming - 1, "x", at, 0.)
                        - (width(incoming) + width(incoming - 1)) * 0.5;
                    // Positive footprint clearance alone is not enough: the
                    // raised side can still project across it along the view ray.
                    let clearance = gap - value(kind, incoming, "depth", at, 28.);
                    assert!(
                        clearance >= 6.,
                        "{kind}-{incoming} overlaps during formation at {at}: projected clearance {clearance}"
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "release-only fixed-clock diagram benchmark; requires headless GPU and fonts"]
    fn diagram_sampling_benchmark() {
        use std::time::Instant;
        let plan = kinograph_opencode_architecture::build_scene().unwrap();
        let mut renderer = pollster::block_on(new_renderer("diagram-perf")).unwrap();
        renderer.set_interactive_preview(true);
        let prepared = PreparedPlan::prepare(plan, Path::new("."), &mut renderer).unwrap();
        let output = std::env::var_os("KINOGRAPH_DIAGRAM_PERF").map(std::path::PathBuf::from);
        if let Some(path) = &output {
            std::fs::create_dir_all(path).unwrap();
        }
        let save = |name: &str, pixels: &[u8]| {
            if let Some(path) = &output {
                let file = std::fs::File::create(path.join(format!("{name}.png"))).unwrap();
                let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 1920, 1080);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(pixels)
                    .unwrap();
            }
        };
        let percentile = |samples: &[f64], fraction: f64| {
            let mut sorted = samples.to_vec();
            sorted.sort_by(f64::total_cmp);
            sorted[((sorted.len() - 1) as f64 * fraction).round() as usize]
        };
        let mut runs = Vec::new();
        for run in 0..8 {
            let mut playback = prepared.playback(false).unwrap();
            let mut render_ms = Vec::new();
            let mut navigation_ms = Vec::new();
            for frame in 0..120 {
                let now = Duration::from_secs_f64(frame as f64 / 60.);
                if frame % 15 == 0 {
                    let action = [
                        PlaybackCommand::Next,
                        PlaybackCommand::Last,
                        PlaybackCommand::Previous,
                        PlaybackCommand::First,
                    ][frame / 15 % 4];
                    let start = Instant::now();
                    playback.command(action, now);
                    navigation_ms.push(start.elapsed().as_secs_f64() * 1000.);
                }
                let start = Instant::now();
                let sample = playback.sample(now);
                let timeline = playback.timeline();
                // Include the worker's visual key, not window acquisition/upload.
                std::hint::black_box(
                    prepared
                        .visual_sample_key_using(sample.at_nanos as f64 / 1e9, &timeline)
                        .unwrap(),
                );
                let pixels = prepared
                    .render_sample_using(&mut renderer, sample.at_nanos as f64 / 1e9, &timeline)
                    .unwrap();
                render_ms.push(start.elapsed().as_secs_f64() * 1000.);
                if run == 1 {
                    save(&format!("native-{frame:03}"), &pixels);
                }
                std::hint::black_box(pixels);
            }
            if run > 0 {
                runs.push(serde_json::json!({"renderMedianMs":percentile(&render_ms,0.5), "renderP95Ms":percentile(&render_ms,0.95), "navigationMedianMs":percentile(&navigation_ms,0.5), "renderMs":render_ms}));
            }
        }
        let medians = runs
            .iter()
            .map(|run| run["renderMedianMs"].as_f64().unwrap())
            .collect::<Vec<_>>();
        let median = percentile(&medians, 0.5);
        let mad = percentile(
            &medians
                .iter()
                .map(|value| (value - median).abs())
                .collect::<Vec<_>>(),
            0.5,
        );
        let report = serde_json::json!({"size":[1920,1080],"temporalSamples":1,"framesPerRun":120,"warmupRuns":1,"medianMs":median,"madMs":mad,"runs":runs});
        println!("METRIC diagram_sample_median_ms={median:.4} mad_ms={mad:.4}");
        if let Some(path) = &output {
            std::fs::write(
                path.join("timing.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
        // Cold theme changes and arbitrary authored times are separate from timing.
        // Save in a deliberately non-monotonic order to catch stale visual caches.
        for (theme_index, theme) in crate::render::Theme::ALL
            .into_iter()
            .chain([crate::render::Theme::Original])
            .enumerate()
        {
            renderer.set_theme(theme);
            for (index, at) in [
                11., 0., 3.12, 9.16, 6.06, 3.05, 9.4, 6.18, 8., 9.8, 3.8, 6.4, 9.22, 3.35, 9.04,
            ]
            .into_iter()
            .enumerate()
            {
                save(
                    &format!("theme-{theme_index}-{index:02}"),
                    &prepared.render_sample(&mut renderer, at).unwrap(),
                );
            }
        }
    }

    #[test]
    fn diagram_preflight_rejects_unknown_ports_channels_and_waits() {
        let scene = kinograph_opencode_architecture::build_scene().unwrap();
        validate_renderer_plan(&scene).unwrap();
        for mutation in 0..5 {
            let mut bad = scene.clone();
            let mut p: DiagramPlan = serde_json::from_value(bad.actors[0].data.clone()).unwrap();
            match mutation {
                0 => p.nodes[1].id = p.nodes[0].id.clone(),
                1 => p.links[0].to.node = "missing".into(),
                2 => p.delays[0].property = "missing".into(),
                3 => p.bounds[2] = f32::MAX,
                _ => bad.continuous_channels[0].property = "node.client-0.typo".into(),
            }
            bad.actors[0].data = serde_json::to_value(p).unwrap();
            assert!(
                validate_renderer_plan(&bad).is_err(),
                "accepted bad case {mutation}"
            );
        }
        let mut duplicate = scene.clone();
        let mut actor = scene.actors[0].clone();
        actor.id = "second-diagram".into();
        duplicate.actors.push(actor);
        assert!(validate_renderer_plan(&duplicate).is_err());
    }

    #[test]
    #[ignore = "requires headless GPU; attached diagrams retain client pixels and redirect all channels continuously"]
    fn diagram_native_export_and_interrupted_navigation_agree() {
        for slide in kinograph_opencode_architecture::build_deck()
            .unwrap()
            .slides
        {
            let plan = slide.plan;
            let mut renderer = pollster::block_on(new_renderer("daemon-merge-proof")).unwrap();
            let prepared = PreparedPlan::prepare(plan, Path::new("."), &mut renderer).unwrap();
            let mut playback = prepared.playback(false).unwrap();
            playback.command(PlaybackCommand::Next, Duration::ZERO);
            for at in [0., 0.05, 0.06, 0.11, 0.12, 0.18, 0.35, 0.8] {
                let sample = playback.sample(Duration::from_secs_f64(at));
                let interactive = prepared
                    .render_sample_using(
                        &mut renderer,
                        sample.at_nanos as f64 / 1e9,
                        &playback.timeline(),
                    )
                    .unwrap();
                let export = prepared.render_sample(&mut renderer, 3. + at).unwrap();
                assert_eq!(interactive, export, "native/export mismatch at {at}");
            }
            if prepared.plan.id == "daemon-isometric" {
                let mut skipped = prepared.playback(false).unwrap();
                skipped.command(PlaybackCommand::Last, Duration::ZERO);
                let width = |timeline: &kinograph::timeline::Timeline, at| {
                    prepared.property_value(
                        timeline,
                        "diagram",
                        "node.server-2.width-reveal",
                        at,
                        -1.,
                    )
                };
                assert_eq!(
                    width(&skipped.timeline(), 0.119),
                    8. / 300.,
                    "a skipped merge must retain the server's width wait"
                );
                assert!(width(&skipped.timeline(), 0.121) > 8. / 300.);
                skipped.command(PlaybackCommand::First, Duration::from_millis(50));
                assert_eq!(
                    width(&skipped.timeline(), 0.3),
                    8. / 300.,
                    "cancelled width entrance must not start later"
                );
            }
            let held = prepared.render_sample(&mut renderer, 8.).unwrap();
            assert!(
                held[..120 * 1920 * 4]
                    .chunks_exact(4)
                    .all(|pixel| pixel == [5, 5, 5, 255]),
                "bare diagram must have no header/frame/dots"
            );
            for at in [9., 9.04, 9.1, 9.16, 9.25, 9.4, 9.8, 11.] {
                let pixels = prepared.render_sample(&mut renderer, at).unwrap();
                if prepared.plan.id == "daemon-merge" {
                    assert_eq!(
                        &held[384 * 1920 * 4..494 * 1920 * 4],
                        &pixels[384 * 1920 * 4..494 * 1920 * 4],
                        "retained clients changed during merge at {at}"
                    );
                }
            }
            playback.command(PlaybackCommand::Next, Duration::from_secs(2));
            playback.sample(Duration::from_secs(4));
            playback.command(PlaybackCommand::Last, Duration::from_secs(5));
            let sample = playback.sample(Duration::from_secs(5));
            assert_eq!(
                playback
                    .pending_starts(Duration::from_nanos(sample.at_nanos))
                    .count,
                5
            );
            for (millis, command) in [
                (5040, PlaybackCommand::Previous),
                (5080, PlaybackCommand::Last),
                (5100, PlaybackCommand::First),
                (5160, PlaybackCommand::Last),
            ] {
                let now = Duration::from_millis(millis);
                let sample = playback.sample(now);
                let t = sample.at_nanos as f64 / 1e9;
                let old = playback.timeline();
                let before = prepared
                    .render_sample_using(&mut renderer, t, &old)
                    .unwrap();
                playback.command(command, now);
                let new = playback.timeline();
                for c in &prepared.plan.continuous_channels {
                    let p = PropertyId::new(&c.id);
                    assert_eq!(
                        old.sample_at(&p, t),
                        new.sample_at(&p, t),
                        "lost position/velocity: {}",
                        c.id
                    );
                }
                assert_eq!(
                    before,
                    prepared
                        .render_sample_using(&mut renderer, t, &new)
                        .unwrap(),
                    "pixel jump at {millis}"
                );
                if millis == 5040 {
                    assert_eq!(
                        playback
                            .pending_starts(Duration::from_nanos(sample.at_nanos))
                            .count,
                        0
                    );
                }
            }
            let timeline = playback.timeline();
            let a = prepared
                .render_sample_using(&mut renderer, 4., &timeline)
                .unwrap();
            let _ = prepared
                .render_sample_using(&mut renderer, 2.5, &timeline)
                .unwrap();
            assert_eq!(
                a,
                prepared
                    .render_sample_using(&mut renderer, 4., &timeline)
                    .unwrap(),
                "out-of-order sample residue"
            );
            if let Some(directory) = std::env::var_os("KINOGRAPH_DIAGRAM_ARTIFACTS") {
                let directory = std::path::PathBuf::from(directory).join(&prepared.plan.id);
                std::fs::create_dir_all(&directory).unwrap();
                let mut clock = prepared.playback(false).unwrap();
                let mut video = crate::encode::FfmpegEncoder::start_with_media(
                    &directory.join("interruptions.mp4"),
                    crate::encode::VideoSpec {
                        width: 1920,
                        height: 1080,
                        fps: 60,
                    },
                    &[],
                )
                .unwrap();
                for frame in 0..132 {
                    let now = Duration::from_secs_f64(frame as f64 / 60.);
                    let action = match frame {
                        0 => Some(PlaybackCommand::Last),
                        8 => Some(PlaybackCommand::Previous),
                        14 => Some(PlaybackCommand::Last),
                        20 => Some(PlaybackCommand::First),
                        26 => Some(PlaybackCommand::Last),
                        _ => None,
                    };
                    if let Some(action) = action {
                        clock.command(action, now);
                    }
                    let sample = clock.sample(now);
                    video
                        .write_frame(
                            &prepared
                                .render_sample_using(
                                    &mut renderer,
                                    sample.at_nanos as f64 / 1e9,
                                    &clock.timeline(),
                                )
                                .unwrap(),
                        )
                        .unwrap();
                }
                video.finish().unwrap();
            }
        }
    }
}

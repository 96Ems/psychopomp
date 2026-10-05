use super::*;
use crate::{
    anchor::Edge,
    author::{SECOND, seconds},
};

fn placement(at: u64) -> MediaPlan {
    media("clip", "clip.mp4", at, at + 10 * SECOND)
}

/// Source seconds at plan second `t` for a clip trimmed to 2..6 s and
/// placed at 1 s.
fn source_at(clip: &Clip, t: f64) -> f64 {
    let placement = placement(SECOND);
    let playhead = clip.natural_nanos(&placement, seconds(t));
    clip.source_nanos(clip.trim_nanos().unwrap(), playhead) as f64 / 1e9
}

#[test]
fn trims_hold_their_ends_and_rates_scale_the_playhead() {
    let clip = Clip::new("clip").trimmed(2.0, 6.0);
    assert_eq!(
        source_at(&clip, 0.0),
        2.0,
        "holds the first frame before it starts"
    );
    assert_eq!(source_at(&clip, 1.0), 2.0);
    assert_eq!(source_at(&clip, 2.5), 3.5);
    assert_eq!(
        source_at(&clip, 9.0),
        6.0,
        "holds the last frame after it ends"
    );
    let fast = clip.clone().at_rate(2.0);
    assert_eq!(source_at(&fast, 2.5), 5.0);
    assert_eq!(source_at(&fast, 4.0), 6.0);
    let slow = clip.clone().at_rate(0.25);
    assert_eq!(source_at(&slow, 5.0), 3.0);
}

#[test]
fn loops_wrap_bounces_mirror_and_reverse_runs_backward() {
    let clip = Clip::new("clip").trimmed(2.0, 6.0);
    let looping = clip.clone().looping();
    assert_eq!(
        source_at(&looping, 5.0),
        2.0,
        "one full pass wraps to the start"
    );
    assert_eq!(source_at(&looping, 6.5), 3.5);
    let bouncing = clip.clone().bouncing();
    assert_eq!(source_at(&bouncing, 5.0), 6.0, "turns around at the end");
    assert_eq!(source_at(&bouncing, 6.0), 5.0);
    assert_eq!(source_at(&bouncing, 9.0), 2.0, "and back at the start");
    assert_eq!(source_at(&bouncing, 10.0), 3.0);
    let reversed = clip.clone().reversed();
    assert_eq!(source_at(&reversed, 1.0), 6.0);
    assert_eq!(source_at(&reversed, 2.0), 5.0);
    assert_eq!(source_at(&reversed, 8.0), 2.0);
    let reversed_loop = clip.clone().reversed().looping();
    assert_eq!(source_at(&reversed_loop, 5.5), 5.5);
    let frozen = clip.frozen(1.25);
    assert_eq!(source_at(&frozen, 0.0), 3.25);
    assert_eq!(source_at(&frozen, 8.0), 3.25);
}

#[test]
fn playheads_are_exact_in_nanoseconds_at_unit_rate() {
    let clip = Clip::new("clip");
    let placement = placement(10_000_000_000);
    // The Video Card's integer mapping: no f32 or f64 rounding at rate 1.
    assert_eq!(
        clip.natural_nanos(&placement, 11_500_000_001),
        1_500_000_001
    );
    assert_eq!(clip.natural_nanos(&placement, 9_000_000_000), 0);
    let trim = (2_000_000_000, 5_000_000_000);
    assert_eq!(clip.source_nanos(trim, 1_500_000_001), 3_500_000_001);
    assert_eq!(clip.source_nanos(trim, 9_000_000_000), 5_000_000_000);
    assert_eq!(
        clip.source_nanos((7, 7), 123),
        7,
        "an empty trim is one frame"
    );
}

#[test]
fn fits_crop_letterbox_or_stretch_and_focus_windows_stay_inside() {
    let wide = [1920.0, 1080.0];
    let (window, content) = Fit::Cover.frame(wide, [400.0, 400.0]);
    assert_eq!(window, [420.0, 0.0, 1080.0, 1080.0]);
    assert_eq!(content, [0.0, 0.0, 400.0, 400.0]);
    let (window, _) = Fit::Cover.frame([1000.0, 1000.0], [400.0, 200.0]);
    assert_eq!(window, [0.0, 250.0, 1000.0, 500.0]);
    let (window, content) = Fit::Contain.frame(wide, [400.0, 400.0]);
    assert_eq!(window, [0.0, 0.0, 1920.0, 1080.0]);
    assert_eq!(content, [0.0, 87.5, 400.0, 225.0]);
    let (_, content) = Fit::Contain.frame([500.0, 1000.0], [400.0, 400.0]);
    assert_eq!(content, [100.0, 0.0, 200.0, 400.0]);
    assert_eq!(
        Fit::Fill.frame(wide, [10.0, 90.0]).0,
        [0.0, 0.0, 1920.0, 1080.0]
    );

    let window = [100.0, 0.0, 800.0, 400.0];
    assert_eq!(focus_window(window, [0.5, 0.5], 1.0), window);
    assert_eq!(
        focus_window(window, [0.5, 0.5], 0.5),
        [300.0, 100.0, 400.0, 200.0]
    );
    assert_eq!(
        focus_window(window, [1.0, 0.0], 0.5),
        [500.0, 0.0, 400.0, 200.0],
        "clamped inside the window"
    );
    let (center, size) = focus_on([0.5, 0.25, 0.5, 0.25]);
    assert_eq!((center, size), ([0.75, 0.375], 0.5));
}

#[test]
fn treatments_desaturate_tint_and_dim() {
    let orange = [1.0, 0.5, 0.0];
    assert_eq!(Treatment::NONE.apply(orange, [0.0, 0.0, 1.0]), orange);
    let gray = Treatment {
        saturation: 0.0,
        ..Treatment::NONE
    }
    .apply(orange, [0.0; 3]);
    assert!(gray.iter().all(|v| (v - gray[0]).abs() < 1e-6));
    let blue = Treatment {
        saturation: 0.0,
        tint: 1.0,
        dim: 0.0,
    }
    .apply(orange, [0.2, 0.4, 1.0]);
    assert!(blue[2] > blue[1] && blue[1] > blue[0]);
    let dark = Treatment {
        dim: 0.5,
        ..Treatment::NONE
    }
    .apply(orange, [0.0; 3]);
    assert_eq!(dark, [0.5, 0.25, 0.0]);
}

fn actor(clip: Clip) -> (PlanBuilder, FootageActor) {
    let mut scene = PlanBuilder::new("footage", 10 * SECOND);
    let plan = FootagePlan::new(clip, [960.0, 540.0], [480.0, 270.0]);
    let actor = FootageActor::declare(
        &mut scene,
        "wall",
        &plan,
        media("clip", "clip.mp4", SECOND, 10 * SECOND),
    )
    .unwrap();
    (scene, actor)
}

fn playhead(scene: &PlanBuilder, actor: &FootageActor, t: f64) -> (f32, f32) {
    actor.playhead().at(scene, seconds(t))
}

#[test]
fn freezing_holds_and_playing_resumes_from_the_held_frame() {
    let (mut scene, actor) = actor(Clip::new("clip"));
    assert_eq!(playhead(&scene, &actor, 0.5), (0.0, 0.0), "not started");
    assert_eq!(
        playhead(&scene, &actor, 3.0),
        (2.0, 1.0),
        "natural playback"
    );
    actor.freeze(&mut scene, seconds(3.0));
    actor.play(&mut scene, seconds(5.0), 1.0);
    for (t, expected) in [(2.0, 1.0), (3.0, 2.0), (4.0, 2.0), (4.99, 2.0), (6.0, 3.0)] {
        let (position, _) = playhead(&scene, &actor, t);
        assert!((position - expected).abs() < 1e-4, "{t}: {position}");
    }
    let plan = scene.finish().unwrap();
    assert!(
        plan.continuous_channels
            .iter()
            .all(|channel| FOOTAGE_CHANNELS.contains(&channel.property.as_str()))
    );
}

#[test]
fn ramps_change_speed_smoothly_and_keep_the_new_rate() {
    let (mut scene, actor) = actor(Clip::new("clip"));
    actor.freeze(&mut scene, seconds(2.0));
    let end = actor.ramp(&mut scene, seconds(3.0), 1.0, 2.0);
    assert_eq!(end, seconds(4.0));
    let (start, rate) = playhead(&scene, &actor, 3.0);
    assert!(
        (start - 1.0).abs() < 1e-4 && rate.abs() < 1e-3,
        "leaves from rest"
    );
    let (middle, rate) = playhead(&scene, &actor, 3.5);
    assert!(
        (rate - 1.0).abs() < 1e-3,
        "half-way to double speed: {rate}"
    );
    assert!((middle - 1.25).abs() < 1e-3, "{middle}");
    let (arrived, rate) = playhead(&scene, &actor, 4.0);
    assert!((arrived - 2.0).abs() < 1e-3 && (rate - 2.0).abs() < 1e-3);
    let (later, rate) = playhead(&scene, &actor, 5.0);
    assert!((later - 4.0).abs() < 1e-3 && (rate - 2.0).abs() < 1e-3);
    // Slowing down from double speed to a quarter.
    let end = actor.ramp(&mut scene, seconds(6.0), 2.0, 0.25);
    let (_, rate) = playhead(&scene, &actor, 7.0);
    assert!((rate - 1.125).abs() < 1e-3);
    let (_, rate) = playhead(&scene, &actor, 9.0);
    assert!((rate - 0.25).abs() < 1e-3 && end == seconds(8.0));
    scene.finish().unwrap();
}

#[test]
fn stutters_repeat_a_beat_and_retimes_scrub_and_hold() {
    let (mut scene, actor) = actor(Clip::new("clip"));
    let end = actor.stutter(&mut scene, seconds(2.0), 0.25, 3);
    assert_eq!(end, seconds(2.75));
    for (t, expected) in [
        (2.1, 1.1),
        (2.35, 1.1),
        (2.6, 1.1),
        (2.8, 1.3),
        (3.25, 1.75),
    ] {
        let (position, _) = playhead(&scene, &actor, t);
        assert!((position - expected).abs() < 1e-3, "{t}: {position}");
    }
    let end = actor.retime(&mut scene, seconds(4.0), 0.0, 1.0, Ease::Smootherstep);
    assert_eq!(end, seconds(5.0));
    assert!(
        playhead(&scene, &actor, 5.5).0.abs() < 1e-6,
        "rewound and held"
    );
    scene.finish().unwrap();
}

#[test]
fn stage_playheads_write_the_elements_time_channel() {
    let mut scene = PlanBuilder::new("stage", 4 * SECOND);
    let stage: crate::stage::StagePlan = serde_json::from_value(serde_json::json!({
        "elements": [{
            "kind": "footage", "id": "tv", "at": [960, 540, -200], "size": [480, 270],
            "clip": { "media": "clip", "repeat": "loop" }, "mask": { "shape": "circle" }
        }]
    }))
    .unwrap();
    let stage_actor = crate::stage::StageActor::declare(&mut scene, "stage", &stage).unwrap();
    let placement = media("clip", "clip.mp4", 0, 4 * SECOND);
    scene.media(placement.clone());
    let playhead = stage_actor.footage_playhead("tv", &placement).unwrap();
    assert_eq!(playhead.property(), "tv.time");
    playhead.freeze(&mut scene, SECOND);
    let plan = scene.finish().unwrap();
    assert!(
        plan.continuous_channels
            .iter()
            .any(|channel| channel.id == "stage.tv.time")
    );
    assert!(stage.accepts("tv.time") && stage.accepts("tv.saturation"));
    assert!(!stage.accepts("tv.tilt-x"));
    assert!(stage_actor.footage_playhead("missing", &placement).is_err());
}

#[test]
fn plans_round_trip_compactly_and_reject_nonsense() {
    let plan = FootagePlan::new(Clip::new("clip"), [960.0, 540.0], [480.0, 270.0]);
    assert_eq!(
        serde_json::to_value(&plan).unwrap(),
        serde_json::json!({
            "clip": { "media": "clip" },
            "center": [960.0, 540.0],
            "size": [480.0, 270.0]
        })
    );
    let rich =
        plan.clone()
            .titled("trace.png")
            .anchor(AnchorPlan::stage("card", "card", Edge::Top));
    let json = serde_json::to_value(&rich).unwrap();
    assert_eq!(
        json["mask"],
        serde_json::json!({ "shape": "rect", "radius": 18.0 })
    );
    assert_eq!(serde_json::from_value::<FootagePlan>(json).unwrap(), rich);
    assert_eq!(rich.card_size(), [480.0, 270.0 + TITLE_BAR]);
    assert!(rich.accepts("anchor.card") && rich.accepts("time") && !rich.accepts("z"));

    for broken in [
        FootagePlan::new(Clip::new("clip").at_rate(0.0), [0.0; 2], [100.0; 2]),
        FootagePlan::new(Clip::new("clip").trimmed(3.0, 1.0), [0.0; 2], [100.0; 2]),
        FootagePlan::new(Clip::new("clip"), [0.0; 2], [2.0, 100.0]),
        FootagePlan::new(Clip::new("clip"), [0.0; 2], [100.0; 2]).polygon([[0.0, 0.0], [1.0, 0.0]]),
        FootagePlan::new(Clip::new("clip"), [0.0; 2], [100.0; 2])
            .circle()
            .titled("no"),
        FootagePlan {
            title: Some("bare".into()),
            ..plan.clone()
        },
    ] {
        assert!(broken.validate().is_err(), "{broken:?}");
    }
    let mut scene = PlanBuilder::new("footage", SECOND);
    let audio = MediaPlan {
        kind: MediaKindPlan::Audio,
        ..media("clip", "clip.wav", 0, SECOND)
    };
    assert!(FootageActor::declare(&mut scene, "wall", &plan, audio).is_err());
    assert!(is_sequence(Path::new("frames/%04d.png")) && is_sequence(Path::new("f%d.jpg")));
    assert!(!is_sequence(Path::new("100%.png")) && !is_sequence(Path::new("clip.mp4")));
}

#[test]
fn audio_follows_the_trim_start_and_loops() {
    let video = media("clip", "clip.mp4", SECOND, 10 * SECOND);
    let clip = Clip::new("clip").trimmed(2.0, 5.0);
    let once = audio("clip-audio", &clip, &video, 10 * SECOND, -6.0).unwrap();
    assert_eq!(once.len(), 1);
    assert_eq!(
        (once[0].source_start_nanos, once[0].source_end_nanos),
        (2 * SECOND, 5 * SECOND)
    );
    assert_eq!(once[0].timeline_start_nanos, SECOND);
    assert!(matches!(once[0].kind, MediaKindPlan::Audio) && once[0].gain_db == -6.0);
    let looped = audio(
        "clip-audio",
        &clip.clone().looping(),
        &video,
        10 * SECOND,
        0.0,
    )
    .unwrap();
    assert_eq!(looped.len(), 3);
    assert_eq!(looped[2].timeline_start_nanos, 7 * SECOND);
    assert_eq!(looped[2].timeline_end_nanos, 10 * SECOND);
    assert_eq!(looped[1].id, "clip-audio#1");
    assert!(audio("a", &clip.clone().at_rate(2.0), &video, 10 * SECOND, 0.0).is_err());
    assert!(audio("a", &Clip::new("clip"), &video, 10 * SECOND, 0.0).is_err());
}

#[test]
fn probes_read_size_rate_alpha_and_audio() {
    let report = serde_json::json!({
        "streams": [
            { "codec_type": "video", "codec_name": "vp9", "width": 640, "height": 360,
              "r_frame_rate": "30000/1001", "pix_fmt": "yuv420p", "tags": { "alpha_mode": "1" } },
            { "codec_type": "audio", "codec_name": "opus" }
        ],
        "format": { "duration": "4.250000" }
    });
    let probe = parse_probe(&report).unwrap();
    assert_eq!(probe.size, [640, 360]);
    assert!((probe.fps - 29.97).abs() < 0.01);
    assert!(probe.alpha && probe.audio && probe.vpx_alpha == Some("libvpx-vp9"));
    assert_eq!(probe.seconds, 4.25);
    let still = serde_json::json!({
        "streams": [{ "codec_type": "video", "codec_name": "png", "width": 64, "height": 32,
                      "r_frame_rate": "25/1", "pix_fmt": "rgba" }],
        "format": {}
    });
    let probe = parse_probe(&still).unwrap();
    assert!(probe.alpha && !probe.audio && probe.vpx_alpha.is_none());
    assert_eq!(probe.aspect(), 2.0);
    assert!(parse_probe(&serde_json::json!({ "streams": [] })).is_err());
}

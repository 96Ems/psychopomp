//! Footage showroom: images, videos, and image sequences as scene material.
//! A masonry wall of stills and clips is tossed in with motion blur; one clip
//! steps forward while the rest recede to gray reference footage, freezes,
//! ramps to triple speed, and stutters. On a Stage, footage sits at depths
//! the camera swings around (parallax and depth of field), a wire carries a
//! packet into a screen recording, and a circular clip rides a card. Then a
//! pile of prints is dealt and drawn out into a filmstrip.
use anyhow::Result;
use psychopomp::{
    anchor::{AnchorPlan, Edge},
    author::{PlanBuilder, seconds},
    caption::{CaptionActor, CaptionAlign, CaptionPlan, CaptionSpanPlan},
    footage::{
        self, Clip, FootageActor, FootagePlan, Treatment,
        layout::{self, Tile},
    },
    math::{easing::Ease, random::hash},
    plan::{MediaPlan, ReelPlan, ScenePlan},
    stage::{Camera, Move, StageActor, StagePlan},
    tone::Tone,
};

const MS: u64 = 1_000_000;

/// A source, relative to `target/` where the plans are written, and its
/// pixel size (as `footage::probe` reports it), for laying it out.
struct Source {
    path: &'static str,
    size: [f32; 2],
    still: bool,
}

impl Source {
    fn aspect(&self) -> f32 {
        self.size[0] / self.size[1]
    }

    /// Its placement, available for the whole scene.
    fn media(&self, id: &str, scene: &PlanBuilder) -> MediaPlan {
        let until = scene.duration_nanos();
        if self.still {
            footage::still(id, self.path, 0, until)
        } else {
            footage::media(id, self.path, 0, until)
        }
    }
}

const RECORDING: Source = Source {
    path: "../assets/opencode-v2-session-tool/live-hot-reload.mp4",
    size: [1120.0, 640.0],
    still: false,
};
const SPLIT: Source = Source {
    path: "../assets/opencode-v2-session-tool/max-hot-reload-split.mp4",
    size: [1920.0, 760.0],
    still: false,
};
const MISSILES: Source = Source {
    path: "../assets/opencode-hot-reload/fire-the-missiles.mp4",
    size: [1200.0, 720.0],
    still: false,
};
const PLUGIN: Source = Source {
    path: "../assets/opencode-v2-session-tool/plugin-created.png",
    size: [1120.0, 640.0],
    still: true,
};
const TOOL: Source = Source {
    path: "../assets/opencode-v2-session-tool/tool-executed.png",
    size: [1120.0, 640.0],
    still: true,
};
const TRACE: Source = Source {
    path: "../assets/anchors/trace.png",
    size: [720.0, 420.0],
    still: true,
};
const MANDELBROT: Source = Source {
    path: "../assets/footage/mandelbrot.mp4",
    size: [640.0, 360.0],
    still: false,
};
const COUNTDOWN: Source = Source {
    path: "../assets/footage/countdown.mp4",
    size: [640.0, 360.0],
    still: false,
};
const GRADIENT: Source = Source {
    path: "../assets/footage/gradient.mp4",
    size: [640.0, 360.0],
    still: false,
};
/// Glowing beads on a ring, with alpha (VP9 in WebM).
const ORBIT: Source = Source {
    path: "../assets/footage/orbit.webm",
    size: [256.0, 256.0],
    still: false,
};
/// A cellular automaton as a PNG image sequence.
const LIFE: Source = Source {
    path: "../assets/footage/life/frame-%03d.png",
    size: [480.0, 272.0],
    still: false,
};

pub fn build_reel() -> Result<ReelPlan> {
    ReelPlan::dipped(
        "footage",
        vec![build_wall()?, build_stage()?, build_pile()?],
        600 * MS,
    )
}

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

/// A caption at the bottom of the frame, shown from `from` until `until`.
fn caption(
    scene: &mut PlanBuilder,
    id: &str,
    spans: Vec<CaptionSpanPlan>,
    from: u64,
    until: u64,
) -> Result<()> {
    let mut caption = CaptionActor::declare(
        scene,
        id,
        &CaptionPlan::line([960.0, 1010.0], 24.0, spans).aligned(CaptionAlign::Center),
    )?;
    caption.show(scene, from);
    caption.hide(scene, until);
    Ok(())
}

/// One piece of the wall: its source, how its clip plays, and whether it is
/// framed as a card.
struct Piece {
    id: &'static str,
    source: Source,
    clip: fn(Clip) -> Clip,
    framed: bool,
}

/// Twelve stills and clips in masonry columns, tossed in as a ripple from
/// the middle. The countdown steps forward and plays with time while the
/// rest recede into gray reference footage, then a still drifts.
pub fn build_wall() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("footage-wall", seconds(11.5));
    let pieces = [
        Piece {
            id: "plugin",
            source: PLUGIN,
            clip: |c| c,
            framed: true,
        },
        Piece {
            id: "mandelbrot",
            source: MANDELBROT,
            clip: Clip::looping,
            framed: false,
        },
        Piece {
            id: "split",
            source: SPLIT,
            clip: |c| c.trimmed(2.0, 14.0),
            framed: true,
        },
        Piece {
            id: "trace",
            source: TRACE,
            clip: |c| c,
            framed: true,
        },
        Piece {
            id: "life",
            source: LIFE,
            clip: |c| c.looping().decoded_at(12),
            framed: false,
        },
        Piece {
            id: "missiles",
            source: MISSILES,
            clip: |c| c,
            framed: false,
        },
        Piece {
            id: "gradient",
            source: GRADIENT,
            clip: Clip::bouncing,
            framed: false,
        },
        Piece {
            id: "tool",
            source: TOOL,
            clip: |c| c,
            framed: true,
        },
        Piece {
            id: "mandelbrot-reversed",
            source: MANDELBROT,
            clip: |c| c.reversed().looping(),
            framed: false,
        },
        Piece {
            id: "recording",
            source: RECORDING,
            clip: |c| c,
            framed: true,
        },
        Piece {
            id: "orbit",
            source: ORBIT,
            clip: Clip::looping,
            framed: false,
        },
        Piece {
            id: "countdown",
            source: COUNTDOWN,
            clip: |c| c,
            framed: true,
        },
    ];
    let aspects = pieces
        .iter()
        .map(|piece| piece.source.aspect())
        .collect::<Vec<_>>();
    let tiles = layout::masonry([150.0, 90.0, 1620.0, 880.0], &aspects, 4, 22.0);
    let mut actors = Vec::new();
    for (piece, tile) in pieces.iter().zip(&tiles) {
        // Two pieces may share one file: each clip names its own placement.
        let media = piece.source.media(&format!("{}-media", piece.id), &scene);
        let clip = (piece.clip)(Clip::new(&media.id));
        let mut plan = FootagePlan::in_tile(clip, *tile);
        plan = if piece.framed {
            plan.framed()
        } else {
            plan.rounded(12.0)
        };
        actors.push(FootageActor::declare(&mut scene, piece.id, &plan, media)?);
    }

    // A ripple out from the middle; each print flies in from beyond its
    // own side of the frame, turning.
    let center = [960.0, 540.0];
    for (rank, &index) in layout::by_distance(&tiles, center).iter().enumerate() {
        let tile = tiles[index];
        let outward = [tile.center[0] - center[0], tile.center[1] - center[1]];
        let length = outward[0].hypot(outward[1]).max(1.0);
        let reach = 760.0 + 260.0 * hash(index as u32, 11);
        let from = [
            outward[0] / length * reach,
            outward[1] / length * reach + 240.0,
        ];
        let spin = (hash(index as u32, 5) - 0.5) * 1.4;
        actors[index].toss_in(&mut scene, 250 * MS + rank as u64 * 70 * MS, from, spin);
    }

    // The countdown steps forward; everything else recedes.
    let hero = actors.last().expect("the countdown is the last piece");
    let place = tiles[pieces.len() - 1];
    let forward = seconds(2.4);
    for actor in &actors[..actors.len() - 1] {
        actor.treat(&mut scene, forward, Treatment::REFERENCE, 0.7);
        actor.to(&mut scene, "defocus", forward, 2.5, 0.7);
    }
    hero.glide(
        &mut scene,
        forward,
        [960.0 - place.center[0], 520.0 - place.center[1]],
        0.9,
    );
    hero.to(&mut scene, "scale", forward, 1000.0 / place.size[0], 0.9);

    let freeze = hero.freeze(&mut scene, seconds(3.7));
    caption(
        &mut scene,
        "frozen",
        vec![
            span("freeze", Tone::Accent),
            span(" the frame showing", Tone::Muted),
        ],
        freeze,
        seconds(4.9),
    )?;
    let ramp = seconds(5.1);
    hero.ramp(&mut scene, ramp, 1.2, 3.0);
    caption(
        &mut scene,
        "ramped",
        vec![
            span("ramp", Tone::Accent),
            span(" from a standstill to 3×", Tone::Muted),
        ],
        ramp,
        seconds(6.9),
    )?;
    let stutter = seconds(7.1);
    let done = hero.stutter(&mut scene, stutter, 0.2, 5);
    hero.play(&mut scene, done, 1.0);
    caption(
        &mut scene,
        "stuttered",
        vec![
            span("stutter", Tone::Accent),
            span(" a fifth of a second, five times", Tone::Muted),
        ],
        stutter,
        seconds(8.5),
    )?;

    let back = seconds(8.7);
    hero.glide(&mut scene, back, [0.0, 0.0], 0.9);
    hero.to(&mut scene, "scale", back, 1.0, 0.9);
    for actor in &actors[..actors.len() - 1] {
        actor.treat(&mut scene, back, Treatment::NONE, 0.7);
        actor.to(&mut scene, "defocus", back, 0.0, 0.7);
    }
    // A Ken Burns move across a still.
    actors[0].drift(
        &mut scene,
        seconds(9.3),
        seconds(11.5),
        [0.0, 0.0, 1.0, 1.0],
        [0.04, 0.06, 0.52, 0.52],
    );

    scene.cue("toss", 0, forward);
    scene.cue("time", forward, back);
    scene.cue("drift", back, scene.duration_nanos());
    Ok(scene.finish()?)
}

/// Footage inside a Stage: a framed screen recording at the focal plane, a
/// fractal far behind, a hexagonal image sequence, a near circular clip,
/// and glowing beads with alpha, all seen through a camera that swings
/// around the screen. A wire carries a packet into the recording, which
/// freezes as it lands; a circular countdown overlay rides the agent card.
pub fn build_stage() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("footage-stage", seconds(11.0));
    let recipe: StagePlan = serde_json::from_value(serde_json::json!({
        "post": { "bloom": 0.3, "grain": 0.02, "vignette": 0.3, "backdrop": 0.2 },
        "elements": [
            { "kind": "footage", "id": "far", "at": [-260, 40, 1200], "size": [960, 540],
              "clip": { "media": "far-media", "repeat": "loop" }, "mask": { "shape": "rect", "radius": 18 } },
            { "kind": "footage", "id": "life", "at": [1880, 170, 560], "size": [560, 317],
              "clip": { "media": "life-media", "repeat": "loop", "fps": 12 },
              "mask": { "shape": "polygon", "points": [[0.25, 0], [0.75, 0], [1, 0.5], [0.75, 1], [0.25, 1], [0, 0.5]] } },
            { "kind": "footage", "id": "screen", "at": [960, 500, 0], "size": [720, 411],
              "clip": { "media": "screen-media" }, "mask": { "shape": "rect", "radius": 14 }, "framed": true },
            { "kind": "card", "id": "agent", "at": [430, 740, -160], "size": [280, 104], "title": "agent",
              "status": [{ "text": "watching the session" }] },
            { "kind": "beam", "id": "watch", "from": "agent", "to": "screen" },
            { "kind": "packet", "id": "reload", "beam": "watch", "label": "reload" },
            { "kind": "footage", "id": "beads", "at": [1190, 290, -260], "size": [220, 220],
              "clip": { "media": "beads-media", "repeat": "loop" } },
            { "kind": "footage", "id": "near", "at": [1420, 770, -520], "size": [230, 230],
              "clip": { "media": "near-media", "repeat": "bounce" }, "mask": { "shape": "circle" }, "framed": true }
        ]
    }))?;
    let mut stage = StageActor::declare(&mut scene, "stage", &recipe)?;
    let mut media = Vec::new();
    for (id, source) in [
        ("far-media", MANDELBROT),
        ("life-media", LIFE),
        ("screen-media", RECORDING),
        ("beads-media", ORBIT),
        ("near-media", GRADIENT),
    ] {
        let placement = source.media(id, &scene);
        scene.media(placement.clone());
        media.push(placement);
    }

    let camera = stage.camera();
    camera.establish(&mut scene, 0, 520.0, 1.8);
    for (index, id) in ["far", "life", "screen", "beads", "near"]
        .into_iter()
        .enumerate()
    {
        let at = 150 * MS + index as u64 * 90 * MS;
        let opacity = stage.channel(&mut scene, &format!("{id}.opacity"), 0.0);
        scene.spring(&opacity, at, 1.0, 0.5, 0.0);
        let scale = stage.channel(&mut scene, &format!("{id}.scale"), 0.9);
        scene.spring(&scale, at, 1.0, 0.8, 0.1);
    }
    let ready = stage.settle_in(&mut scene, "agent", 600 * MS);

    // The countdown, cut to a circle, rides the agent card through every move.
    let pip = FootagePlan::new(Clip::new("pip-media").looping(), [0.0, 0.0], [170.0, 170.0])
        .circle()
        .framed()
        .anchor(AnchorPlan::stage("agent", "agent", Edge::Top).with_offset([0.0, -112.0]));
    let pip_media = COUNTDOWN.media("pip-media", &scene);
    let pip = FootageActor::declare(&mut scene, "pip", &pip, pip_media)?;
    pip.fly_in(&mut scene, ready);

    camera.aperture(&mut scene, seconds(1.6), 0.55, Move::Glide(1.0));
    camera.focus_on(&mut scene, "screen", seconds(1.6), Move::Glide(1.0))?;
    let swung = camera.orbit(
        &mut scene,
        "screen",
        seconds(2.0),
        [-0.34, 0.07],
        Move::Glide(3.6),
    )?;

    let wired = stage.connect(&mut scene, "watch", seconds(3.0), 0.6);
    let landed = stage.send(&mut scene, "reload", wired + 300 * MS, 0.9);
    stage.land(&mut scene, "screen", landed);
    // The recording holds the frame the packet landed on, then plays on.
    let screen = stage.footage_playhead("screen", &media[2])?;
    screen.freeze(&mut scene, landed);
    screen.ramp(&mut scene, landed + seconds(1.2), 0.8, 1.0);

    let pushed = camera.orbit(
        &mut scene,
        "screen",
        swung + 200 * MS,
        [0.26, -0.05],
        Move::Glide(2.6),
    )?;
    camera.push_in(&mut scene, swung + 200 * MS, 240.0, Move::Glide(2.6));
    let level = Camera {
        yaw: 0.0,
        pitch: 0.0,
        ..camera.pose(&scene, pushed)
    };
    let settled = camera.move_to(&mut scene, &level, pushed + 300 * MS, Move::Glide(2.0));
    camera.pull_back(&mut scene, pushed + 300 * MS, 240.0, Move::Glide(2.0));
    camera.frame(
        &mut scene,
        &["screen", "agent"],
        160.0,
        settled,
        Move::Spring(1.0),
    )?;

    scene.cue("establish", 0, seconds(2.0));
    scene.cue("orbit", seconds(2.0), pushed);
    scene.cue("settle", pushed, scene.duration_nanos());
    Ok(scene.finish()?)
}

/// Nine prints dealt into a pile with seeded turns, then drawn out into a
/// filmstrip that slides across the frame.
pub fn build_pile() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("footage-pile", seconds(7.0));
    let sources = [
        RECORDING, MANDELBROT, TRACE, COUNTDOWN, TOOL, GRADIENT, MISSILES, LIFE, PLUGIN,
    ];
    let size = [420.0, 252.0];
    let pile = layout::pile([960.0, 520.0], sources.len(), size, 17, 210.0, 0.3);
    let strip = layout::filmstrip([300.0, 540.0], sources.len(), [300.0, 180.0], 20.0, false);
    let mut prints = Vec::new();
    for (index, (source, tile)) in sources.iter().zip(&pile).enumerate() {
        let media = source.media(&format!("print-{index}-media"), &scene);
        let clip = Clip::new(&media.id).looping();
        let plan = FootagePlan::in_tile(clip, *tile).framed();
        let print = FootageActor::declare(&mut scene, format!("print-{index}"), &plan, media)?;
        // Dealt from above the frame, one after another.
        let from = [
            (hash(index as u32, 3) - 0.5) * 500.0,
            -760.0 - tile.center[1],
        ];
        print.toss_in(
            &mut scene,
            200 * MS + index as u64 * 110 * MS,
            from,
            (hash(index as u32, 9) - 0.5) * 1.6,
        );
        prints.push((print, *tile));
    }
    let draw = seconds(2.9);
    for (index, (print, tile)) in prints.iter().enumerate() {
        let target: Tile = strip[index];
        let at = draw + index as u64 * 55 * MS;
        print.glide(
            &mut scene,
            at,
            [
                target.center[0] - tile.center[0],
                target.center[1] - tile.center[1],
            ],
            0.9,
        );
        let rotation = print.channel(&mut scene, "rotation");
        scene.ease(&rotation, at, -tile.rotation, 0.9, Ease::Smootherstep);
        print.to(&mut scene, "scale", at, target.size[0] / tile.size[0], 0.9);
    }
    // The strip slides left as if pulled through a viewer.
    let slide = seconds(4.6);
    for (index, (print, tile)) in prints.iter().enumerate() {
        let target = strip[index];
        let x = print.channel(&mut scene, "x");
        scene.ease(
            &x,
            slide,
            target.center[0] - tile.center[0] - 760.0,
            2.4,
            Ease::CubicInOut,
        );
    }
    scene.cue("deal", 0, draw);
    scene.cue("strip", draw, scene.duration_nanos());
    Ok(scene.finish()?)
}

/// A measuring wall: twenty playing clips in a grid (four sources, so tiles
/// share decodes), tossed in, then playing for three seconds.
pub fn build_bench() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("footage-bench", seconds(4.0));
    let sources = [MANDELBROT, COUNTDOWN, GRADIENT, RECORDING];
    let tiles = layout::grid([60.0, 60.0, 1800.0, 960.0], 20, 5, 16.0, 16.0 / 9.0);
    for (index, tile) in tiles.iter().enumerate() {
        let source = &sources[index % sources.len()];
        let media = source.media(&format!("bench-{index}-media"), &scene);
        let clip = Clip::new(&media.id)
            .looping()
            .at_rate(0.5 + 0.25 * (index % 4) as f32);
        let plan = FootagePlan::in_tile(clip, *tile).rounded(10.0);
        let tile = FootageActor::declare(&mut scene, format!("bench-{index}"), &plan, media)?;
        let from = [0.0, 900.0 + 40.0 * (index % 5) as f32];
        tile.toss_in(
            &mut scene,
            index as u64 * 25 * MS,
            from,
            (hash(index as u32, 2) - 0.5) * 0.8,
        );
    }
    Ok(scene.finish()?)
}

#[cfg(test)]
mod tests {
    use psychopomp::footage::FOOTAGE_CHANNELS;

    use super::*;

    #[test]
    fn the_reel_validates_and_every_channel_is_known() {
        let reel = build_reel().unwrap();
        assert_eq!(reel.segments.len(), 3);
        for segment in &reel.segments {
            let plan = &segment.plan;
            for channel in &plan.continuous_channels {
                let actor = plan
                    .actors
                    .iter()
                    .find(|a| a.id == channel.actor_id)
                    .unwrap();
                if actor.recipe == footage::FOOTAGE_RECIPE {
                    assert!(
                        FOOTAGE_CHANNELS.contains(&channel.property.as_str())
                            || channel.property.starts_with("anchor."),
                        "{}",
                        channel.id
                    );
                }
            }
        }
        let wall = &reel.segments[0].plan;
        assert!(
            wall.continuous_channels
                .iter()
                .any(|c| c.id == "countdown.time")
        );
        let stage = &reel.segments[1].plan;
        assert!(
            stage
                .continuous_channels
                .iter()
                .any(|c| c.id == "stage.screen.time")
        );
        assert_eq!(build_bench().unwrap().actors.len(), 20);
    }
}

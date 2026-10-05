//! A weird Psychopomp reply film for Dmitriy Kovalenko (`@neogoose_btw`),
//! author of `fframes`, who tweeted at 8:01 AM:
//! "hope this is fframes based not moviepy, remotion or smth🤞"
//!
//! Act 1 (`morning`): A soft ASMR greeting ("Good morning, Dmitriy.") over a
//! waking particle orb. MoviePy and Remotion try to enter and get vaporized
//! by lightning bolts (`zap` + `dissolve`); `fframes` settles in as a fellow
//! Rust brother.
//!
//! Act 2 (`architecture`): A side-by-side Stage comparison of `fframes`
//! (`Scene::render_frame` -> CPU `<svg>` tree -> `svgr / tiny-skia` ->
//! `libavcodec`) versus `psychopomp` (`ScenePlan.json` with closed-form
//! velocity-preserving springs -> `wgpu` + WGSL + `cosmic-text` -> 16-sample
//! temporal shutter -> `ffmpeg` / `winit`). The `wgpu` particle form morphs
//! from a flat 2D plane into a 3D cube, torus, and sphere, whip-pans with
//! shutter blur, detonates in volumetric raymarched combustion ("try putting
//! that in an SVG path, Dmitriy"), and VHS-rewinds back into place.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    callout::{CalloutActor, CalloutAnchorPlan, CalloutPlan, CalloutSide},
    caption::{CaptionAlign, CaptionSpanPlan},
    effects::combustion,
    face::Face,
    math::easing::Ease,
    narration::Reading,
    plan::{ReelPlan, ReelSegmentPlan, ReelTransitionStyle, ScenePlan},
    rolling::{RollingNumberActor, RollingNumberPlan},
    sfx,
    stage::{
        Arrow, BoltEnd, Curve, FormShape, OrbEntrance, StageActor, StageElement, StagePlan,
        StagePost, Waypoint,
    },
    tone::Tone,
};
use psychopomp_media::{Audio, Media, Voice};

/// Kit's ElevenLabs Professional Voice Clone.
const KIT: &str = "8olojUk4IXpvKgaOCHXj";

const MORNING: &str = "[soft, sweet, intimate ASMR whisper] Good morning, Dmitriy. [tender, gentle, smiling] Eight oh one A M. You saw a video from OpenCode... and you crossed your fingers. [whispering, conspiratorial] Please don't be MoviePy. Please don't be Remotion. [warm, admiring] Please... be fframes.";

const CONTRAST: &str = "[warm, brotherly, smiling] Good news, Dmitriy! It is Rust! And every frame is a pure function of time! [sudden, explosive SHOUTING] ANY FRAME! ANY ORDER!! [conversational, curious, leaning in] Wait... is it fframes? No. It's Psychopomp. Here is how our brains split. [crisp, matter-of-fact] In fframes, every frame builds an S V G tree on the C P U, rasterizes it with tiny-skia, and feeds raw bytes into libavcodec. [excited, building, faster] Psychopomp skips the S V G! A Rust program compiles closed-form springs—velocity preserved across every interruption—into a JSON scene plan, and hands it straight to W G P U!";

const COMBUSTION: &str = "[excited, building, getting faster and louder] Because what happens when your diagram needs sixteen-sample temporal motion blur?! Depth-of-field bokeh! Three-D particle morphs! [screaming at the top of his lungs, completely unhinged] OR VOLUMETRIC RAYMARCHED COMBUSTION WHEN THE SERVER EXPLODES?!! [sudden deadpan whisper, soft and sweet] ...try putting that in an S V G path, Dmitriy. [warm, gentle, matter-of-fact] And then you rewind the clock, and every spring keeps its velocity. [tender, smiling] Good morning, Dmitriy. Welcome to Psychopomp.";

const ORB: [f32; 3] = [960.0, 480.0, 0.0];
const MOVIEPY: [f32; 3] = [350.0, 480.0, -30.0];
const REMOTION: [f32; 3] = [1570.0, 480.0, -30.0];
const FFRAMES_HERO: [f32; 3] = [350.0, 480.0, 0.0];
const TWEET: [f32; 3] = [960.0, 825.0, 0.0];

const FF_SCENE: [f32; 3] = [360.0, 290.0, 0.0];
const FF_SVGR: [f32; 3] = [920.0, 290.0, 0.0];
const FF_LIBAV: [f32; 3] = [1480.0, 290.0, 0.0];

const PSY_PLAN: [f32; 3] = [360.0, 685.0, 0.0];
const WGPU_CORE: [f32; 3] = [940.0, 685.0, 0.0];
const PSY_OUT: [f32; 3] = [1500.0, 685.0, 0.0];

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let media = Media::open(&root)?;
    let kit = Voice::eleven(KIT)
        .v4()
        .stability(0.2)
        .similarity(0.65)
        .whisper();
    let morning = media.dialogue("morning", [(&kit, MORNING)])?;
    let contrast = media.dialogue("contrast", [(&kit, CONTRAST)])?;
    let combustion = media.dialogue("combustion", [(&kit, COMBUSTION)])?;
    media.finish()?;

    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "good-morning-dmitriy".to_owned(),
        segments: vec![
            ReelSegmentPlan {
                transition_nanos: 0,
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                transition_wipe: None,
                plan: morning_scene(&morning)?,
            },
            ReelSegmentPlan {
                transition_nanos: seconds(0.65),
                transition_style: ReelTransitionStyle::Dip,
                transition_focus: None,
                transition_wipe: None,
                plan: architecture_scene(&contrast, &combustion)?,
            },
        ],
    };
    reel.validate()?;
    let output = root.join("good-morning-dmitriy.reel.json");
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "wrote {} ({:.1}s)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9
    );
    Ok(())
}

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

fn morning_stage() -> StagePlan {
    let elements = vec![
        StageElement::orb("orb", ORB, 130.0)
            .points(900)
            .tone(Tone::Accent),
        StageElement::ring("halo-1", ORB, 146.0)
            .thickness(1.4)
            .tone(Tone::Accent),
        StageElement::ring("halo-2", ORB, 154.0)
            .thickness(1.4)
            .tone(Tone::Request),
        StageElement::label(
            "greeting",
            [960.0, 170.0, 0.0],
            76.0,
            &[("Good morning, ", Tone::Plain), ("Dmitriy.", Tone::Accent)],
        )
        .face(Face::SerifItalic),
        StageElement::label(
            "handle",
            [960.0, 244.0, 0.0],
            24.0,
            &[
                ("@neogoose_btw", Tone::Request),
                ("  ·  author of ", Tone::Muted),
                ("fframes", Tone::Plain),
            ],
        ),
        StageElement::card("tweet", TWEET, [720.0, 116.0], "@neogoose_btw · 8:01 AM")
            .statuses(&[
                ("hope this is fframes, not moviepy or remotion", Tone::Plain),
                ("eliminating suspects...", Tone::Warning),
                ("fellow rustacean detected", Tone::Success),
            ])
            .tone(Tone::Request),
        StageElement::card("moviepy", MOVIEPY, [310.0, 112.0], "moviepy")
            .statuses(&[
                ("python · frame += 1", Tone::Error),
                ("vaporized", Tone::Muted),
            ])
            .tone(Tone::Error),
        StageElement::card("remotion", REMOTION, [310.0, 112.0], "remotion")
            .statuses(&[
                ("headless chrome · 400 MB", Tone::Warning),
                ("vaporized", Tone::Muted),
            ])
            .tone(Tone::Warning),
        StageElement::Bolt {
            id: "zap-moviepy".into(),
            from: BoltEnd::Element("orb".into()),
            to: BoltEnd::Element("moviepy".into()),
            strikes: 4,
            branching: 0.75,
            tone: Tone::Error,
        },
        StageElement::Bolt {
            id: "zap-remotion".into(),
            from: BoltEnd::Element("orb".into()),
            to: BoltEnd::Element("remotion".into()),
            strikes: 4,
            branching: 0.75,
            tone: Tone::Error,
        },
        StageElement::card("fframes", FFRAMES_HERO, [320.0, 114.0], "fframes")
            .statuses(&[("rust · pure frame(t)", Tone::Request)])
            .tone(Tone::Request),
        StageElement::beam("link-fframes", "fframes", "orb").tone(Tone::Request),
        StageElement::packet("hello-rust", "link-fframes")
            .labeled("fn render_frame")
            .tone(Tone::Request),
    ];
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

fn morning_scene(morning_clip: &Audio) -> Result<ScenePlan> {
    let reading = Reading::new(seconds(0.8), [(morning_clip.clip(), seconds(0.9))]);
    let mut scene = PlanBuilder::new("morning", reading.duration());
    let [m] = reading.place(&mut scene);
    let mut stage = StageActor::declare(&mut scene, "stage", &morning_stage())?;
    let s = &mut stage;
    let sc = &mut scene;

    let entrance = OrbEntrance {
        scale: 0.52,
        scale_seconds: 1.1,
        blur: 12.0,
        blur_seconds: 0.9,
        rotation: -1.8,
        turn_seconds: 1.5,
        fade_seconds: 0.8,
    };
    for (property, initial) in [
        ("camera.z", -140.0),
        ("camera.dof", 0.4),
        ("orb.scale", entrance.scale),
        ("orb.blur", entrance.blur),
        ("orb.rotation", entrance.rotation),
        ("post.vignette", 0.24),
        ("post.exposure", 1.0),
        ("post.bloom", 0.2),
        ("greeting.opacity", 0.0),
        ("greeting.scale", 0.92),
        ("handle.opacity", 0.0),
    ] {
        s.channel(sc, property, initial);
    }

    // Wake the orb in the dark and whisper "Good morning, Dmitriy."
    s.to(sc, "camera.z", 0, 0.0, 2.4);
    s.orb_in(sc, "orb", seconds(0.15), entrance);
    sfx::SPARKLE.play(sc, "wake", seconds(0.2), -15.0);

    let gm = m.at("good morning");
    s.fade_in(sc, "greeting", gm, 1.0, 0.65);
    s.bounce(sc, "greeting.scale", gm, 1.0, 0.9, 0.16);
    let dmitriy = m.at("dimitri");
    s.hit(sc, "orb.pulse", dmitriy, 0.55, 0.0);
    s.halo(sc, [("halo-1", 0.35), ("halo-2", 0.5)], dmitriy, 0.28);
    s.type_in(sc, "handle", dmitriy + seconds(0.15), 38.0);
    sfx::SPARKLE.play(sc, "dmitriy-sparkle", dmitriy, -14.0);

    // 8:01 AM tweet card settles in.
    let tweet_at = m.at("am").saturating_sub(seconds(0.5));
    s.settle_in(sc, "tweet", tweet_at);
    sfx::TICK.play(sc, "tweet-in", tweet_at, -16.0);

    // Fingers crossed: the two suspects appear on the flanks.
    let fingers = m.at("fingers");
    s.settle_in(sc, "moviepy", fingers);
    s.settle_in(sc, "remotion", fingers + seconds(0.14));
    s.to(sc, "tweet.status", fingers, 1.0, 0.35);
    sfx::TICK.play(sc, "suspects-in", fingers, -17.0);

    // "Please don't be MoviePy." -> charge & zap MoviePy, then dissolve it!
    let no_moviepy = m.at("moviepie");
    s.charge(
        sc,
        "orb",
        no_moviepy.saturating_sub(seconds(0.35)),
        0.8,
        0.35,
    );
    let hit_mp = s.zap(sc, "zap-moviepy", no_moviepy);
    s.charge(sc, "orb", hit_mp, 0.0, 0.0);
    s.jolt(sc, hit_mp, [-1.0, 0.2], 0.65);
    s.hit(sc, "post.chroma", hit_mp, 0.28, 0.0);
    s.set(sc, "moviepy.damage", hit_mp, 1.0);
    s.glitch(sc, "moviepy", hit_mp, [6.0, 8.0, 9.0]);
    s.to(sc, "moviepy.status", hit_mp, 1.0, 0.15);
    s.dissolve(sc, "moviepy", hit_mp + seconds(0.18));
    sfx::IMPACT.play(sc, "zap-mp-hit", hit_mp, -9.0);
    sfx::GLITCH.play(sc, "zap-mp-glitch", hit_mp + seconds(0.05), -14.0);

    // "Please don't be Remotion." -> charge & zap Remotion, then dissolve it!
    let no_remotion = m.at("remotion");
    s.charge(
        sc,
        "orb",
        no_remotion.saturating_sub(seconds(0.3)),
        0.8,
        0.3,
    );
    let hit_rm = s.zap(sc, "zap-remotion", no_remotion);
    s.charge(sc, "orb", hit_rm, 0.0, 0.0);
    s.jolt(sc, hit_rm, [1.0, 0.2], 0.65);
    s.hit(sc, "post.chroma", hit_rm, 0.28, 0.0);
    s.set(sc, "remotion.damage", hit_rm, 1.0);
    s.glitch(sc, "remotion", hit_rm, [8.0, 5.0, 7.0]);
    s.to(sc, "remotion.status", hit_rm, 1.0, 0.15);
    s.dissolve(sc, "remotion", hit_rm + seconds(0.18));
    sfx::IMPACT.play(sc, "zap-rm-hit", hit_rm, -9.0);
    sfx::GLITCH.play(sc, "zap-rm-glitch", hit_rm + seconds(0.05), -14.0);

    // "Please... be fframes." -> fframes card settles in warmly and connects!
    let fframes_at = m.at("please be") + seconds(0.35);
    s.settle_in(sc, "fframes", fframes_at);
    let contact = s.connect(sc, "link-fframes", fframes_at + seconds(0.18), 0.55);
    let arrival = s.send(sc, "hello-rust", contact + seconds(0.05), 0.35);
    s.land(sc, "orb", arrival);
    s.to(sc, "tweet.status", fframes_at, 2.0, 0.35);
    sfx::CONFIRM.play(sc, "fframes-confirm", contact, -13.0);

    scene.finish().context("morning")
}

fn architecture_stage() -> StagePlan {
    let elements = vec![
        // Lane headers
        StageElement::label(
            "hdr-fframes",
            [205.0, 195.0, 0.0],
            25.0,
            &[
                ("fframes  ", Tone::Request),
                ("CPU SVG pipeline  ·  @neogoose_btw", Tone::Muted),
            ],
        )
        .align(CaptionAlign::Left),
        StageElement::label(
            "hdr-psy",
            [205.0, 505.0, 0.0],
            25.0,
            &[
                ("psychopomp  ", Tone::Accent),
                ("GPU shader + shutter pipeline", Tone::Plain),
            ],
        )
        .align(CaptionAlign::Left),
        // Top lane: fframes (Scene::render_frame -> svgr / tiny-skia -> libavcodec)
        StageElement::card("ff-scene", FF_SCENE, [310.0, 108.0], "Scene::render_frame")
            .statuses(&[
                ("svg! tree per frame", Tone::Request),
                ("shocked", Tone::Error),
            ])
            .tone(Tone::Request),
        StageElement::card("ff-svgr", FF_SVGR, [320.0, 108.0], "svgr / tiny-skia")
            .statuses(&[
                ("CPU 2D SVG rasterizer", Tone::Request),
                ("svg overflow", Tone::Error),
            ])
            .tone(Tone::Request),
        StageElement::card("ff-libav", FF_LIBAV, [310.0, 108.0], "libavcodec")
            .statuses(&[("ffmpeg-sys C FFI", Tone::Muted), ("dropped", Tone::Error)])
            .tone(Tone::Request),
        StageElement::Path {
            id: "ff-path".into(),
            through: vec![
                Waypoint::Element("ff-scene".into()),
                Waypoint::Element("ff-svgr".into()),
                Waypoint::Element("ff-libav".into()),
            ],
            curve: Curve::Straight,
            corner: 0.0,
            bend: 0.0,
            tone: Tone::Request,
            width: 1.4,
            dash: None,
            arrow: Arrow::End,
        },
        StageElement::packet("ff-pkt-1", "ff-path")
            .labeled("<svg> tree")
            .tone(Tone::Request),
        StageElement::packet("ff-pkt-2", "ff-path")
            .labeled("RGBA buf")
            .tone(Tone::Request),
        // Bottom lane: psychopomp (ScenePlan.json -> wgpu 3D Form -> ffmpeg + winit)
        StageElement::card("psy-plan", PSY_PLAN, [330.0, 114.0], "ScenePlan.json")
            .statuses(&[
                ("closed-form springs x(t), v(t)", Tone::Accent),
                ("shockwave hit", Tone::Error),
                ("velocity preserved", Tone::Success),
            ])
            .tone(Tone::Accent),
        StageElement::Form {
            id: "wgpu".into(),
            at: WGPU_CORE,
            points: 720,
            tone: Tone::Accent,
            tilt: 0.42,
            shapes: vec![
                FormShape::Plane {
                    size: [250.0, 144.0],
                },
                FormShape::Box {
                    size: [176.0, 176.0, 176.0],
                    edges: 0.65,
                },
                FormShape::Torus {
                    radius: 108.0,
                    tube: 38.0,
                },
                FormShape::Sphere { radius: 132.0 },
            ],
        },
        StageElement::label(
            "wgpu-caption",
            [WGPU_CORE[0], 862.0, 0.0],
            23.0,
            &[
                ("wgpu", Tone::Accent),
                (" · WGSL · cosmic-text", Tone::Muted),
            ],
        ),
        StageElement::card("psy-out", PSY_OUT, [320.0, 114.0], "ffmpeg + winit")
            .statuses(&[
                ("16x shutter · 60fps live", Tone::Success),
                ("offline", Tone::Error),
            ])
            .tone(Tone::Success),
        StageElement::beam("psy-in", "psy-plan", "wgpu").tone(Tone::Accent),
        StageElement::beam("psy-emit", "wgpu", "psy-out").tone(Tone::Success),
        StageElement::packet("psy-pkt-1", "psy-in")
            .labeled("x(t), v(t)")
            .tone(Tone::Accent),
        StageElement::packet("psy-pkt-2", "psy-in")
            .labeled("WGSL")
            .tone(Tone::Accent),
        StageElement::packet("psy-pkt-3", "psy-in")
            .labeled("16x shutter")
            .tone(Tone::Accent),
        StageElement::packet("psy-out-1", "psy-emit")
            .labeled("1080p60")
            .tone(Tone::Success),
        StageElement::packet("psy-out-2", "psy-emit")
            .labeled("winit live")
            .tone(Tone::Success),
        StageElement::ring("ripple-1", WGPU_CORE, 148.0)
            .thickness(1.4)
            .tone(Tone::Accent),
        StageElement::ring("ripple-2", WGPU_CORE, 156.0)
            .thickness(1.4)
            .tone(Tone::Success),
        // Shout labels for ANY FRAME! ANY ORDER!
        StageElement::label(
            "shout-frame",
            [960.0, 270.0, 150.0],
            128.0,
            &[("ANY FRAME!", Tone::Plain)],
        ),
        StageElement::label(
            "shout-order",
            [960.0, 745.0, 150.0],
            128.0,
            &[("ANY ORDER!", Tone::Accent)],
        ),
        // Final closing title
        StageElement::label(
            "outro-gm",
            [960.0, 250.0, 0.0],
            68.0,
            &[("Good morning, ", Tone::Plain), ("Dmitriy.", Tone::Accent)],
        )
        .face(Face::SerifItalic),
        StageElement::label(
            "outro-title",
            [960.0, 825.0, 0.0],
            88.0,
            &[("psychopomp", Tone::Accent)],
        ),
        StageElement::label(
            "outro-url",
            [960.0, 905.0, 0.0],
            28.0,
            &[
                ("github.com/kitlangton/", Tone::Muted),
                ("psychopomp", Tone::Plain),
            ],
        ),
    ];
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

fn architecture_scene(contrast_clip: &Audio, combustion_clip: &Audio) -> Result<ScenePlan> {
    let reading = Reading::new(
        seconds(0.45),
        [
            (contrast_clip.clip(), seconds(0.4)),
            (combustion_clip.clip(), seconds(2.2)),
        ],
    );
    let mut scene = PlanBuilder::new("architecture", reading.duration());
    let [c, b] = reading.place(&mut scene);
    let mut stage = StageActor::declare(&mut scene, "stage", &architecture_stage())?;
    let s = &mut stage;
    let sc = &mut scene;

    for (property, initial) in [
        ("camera.z", 40.0),
        ("camera.dof", 0.35),
        ("post.vignette", 0.22),
        ("post.exposure", 1.0),
        ("post.bloom", 0.18),
        ("hdr-fframes.opacity", 0.0),
        ("hdr-psy.opacity", 0.0),
        ("wgpu.opacity", 0.0),
        ("wgpu.scale", 0.7),
        ("wgpu.morph", 3.0),
        ("wgpu-caption.opacity", 0.0),
        ("shout-frame.opacity", 0.0),
        ("shout-frame.scale", 1.65),
        ("shout-order.opacity", 0.0),
        ("shout-order.scale", 1.65),
        ("outro-gm.opacity", 0.0),
        ("outro-title.opacity", 0.0),
        ("outro-url.opacity", 0.0),
        ("ff-path.draw", 0.0),
    ] {
        s.channel(sc, property, initial);
    }

    // Start on the glowing sphere in the center while shouting ANY FRAME! ANY ORDER!
    let rust_at = c.at("rust");
    s.to(sc, "wgpu.opacity", 0, 1.0, 0.45);
    s.bounce(sc, "wgpu.scale", 0, 1.0, 0.8, 0.18);

    let mut clock = RollingNumberActor::declare(
        sc,
        "clock",
        RollingNumberPlan::new([960.0, 115.0], 34.0, "0.00")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![span("f(t)  t = ", Tone::Muted)])
            .suffix(vec![span(" s", Tone::Muted)])
            .chip(),
    )?;
    clock.show(sc, rust_at);
    clock.roll(sc, c.at("pure function"), "1.25")?;

    for (shout, at, direction, value) in [
        ("shout-frame", c.at("any frame"), [1.0, 0.3], "31.40"),
        ("shout-order", c.at("any order"), [-1.0, -0.35], "0.07"),
    ] {
        s.set(sc, &format!("{shout}.opacity"), at, 1.0);
        s.bounce(sc, &format!("{shout}.scale"), at, 1.0, 0.3, 0.28);
        s.jolt(sc, at, direction, 0.95);
        s.hit(sc, "post.chroma", at, 0.32, 0.0);
        s.hit(sc, "post.zoom", at, 0.14, 0.0);
        s.hit(sc, "post.exposure", at, 1.65, 1.0);
        s.hit(sc, "wgpu.pulse", at, 1.0, 0.0);
        clock.roll(sc, at, value)?;
        sfx::IMPACT.play(sc, format!("{shout}-impact"), at, -6.5);
    }

    // Clear the shouts and dolly back into the two-lane architecture comparison!
    let split = c.at("wait");
    for shout in ["shout-frame", "shout-order"] {
        s.to(sc, &format!("{shout}.opacity"), split, 0.0, 0.25);
    }
    s.to(sc, "camera.z", split, -60.0, 1.4);
    // Morph wgpu down to shape 0 (flat 2D plane) so we can morph it to 3D when Psychopomp's turn comes!
    s.morph(sc, "wgpu", split + seconds(0.2), 0, 0.9);

    // Top lane: fframes
    let in_fframes = c.at("every frame builds").saturating_sub(seconds(0.8));
    s.type_in(sc, "hdr-fframes", in_fframes, 48.0);
    s.settle_in(sc, "ff-scene", in_fframes + seconds(0.1));
    s.settle_in(
        sc,
        "ff-svgr",
        c.at("rasterizes").saturating_sub(seconds(0.35)),
    );
    s.settle_in(sc, "ff-libav", c.at("libav").saturating_sub(seconds(0.45)));
    let ff_wired = s.connect(sc, "ff-path", in_fframes + seconds(0.45), 0.95);
    s.relay(sc, "ff-pkt-1", ff_wired, 1.05);
    s.relay(sc, "ff-pkt-2", ff_wired + seconds(0.45), 1.05);
    sfx::TICK.play(sc, "ff-lane-in", in_fframes, -15.0);

    let mut cpu_callout = CalloutActor::declare(
        sc,
        "cpu-callout",
        &CalloutPlan::new(
            CalloutAnchorPlan::Stage {
                id: "ff-svgr".into(),
                element: "ff-svgr".into(),
                edge: CalloutSide::Bottom,
                side: None,
            },
            vec![
                span("CPU raster ", Tone::Request),
                span("· 1 sample/frame", Tone::Muted),
            ],
        )
        .side(CalloutSide::Bottom)
        .elbow()
        .reach(38.0)
        .tone(Tone::Request)
        .chip(),
    )?;
    cpu_callout.show(sc, c.at("tyneskia"));

    // Bottom lane: psychopomp skips the SVG!
    let skips = c.at("skips");
    s.type_in(sc, "hdr-psy", skips, 48.0);
    s.settle_in(sc, "psy-plan", skips + seconds(0.12));
    s.fade_in(sc, "wgpu-caption", skips + seconds(0.25), 1.0, 0.45);
    s.settle_in(sc, "psy-out", skips + seconds(0.3));
    let in_contact = s.connect(sc, "psy-in", skips + seconds(0.4), 0.55);
    let out_contact = s.connect(sc, "psy-emit", skips + seconds(0.65), 0.55);
    sfx::CONFIRM.play(sc, "psy-lane-in", in_contact, -13.0);

    // Closed-form springs: velocity preserved across every interruption!
    let springs = c.at("springs");
    let mut vel_callout = CalloutActor::declare(
        sc,
        "vel-callout",
        &CalloutPlan::new(
            CalloutAnchorPlan::Stage {
                id: "psy-plan".into(),
                element: "psy-plan".into(),
                edge: CalloutSide::Bottom,
                side: None,
            },
            vec![
                span("x(t), v(t) ", Tone::Accent),
                span("preserved on interrupt", Tone::Plain),
            ],
        )
        .side(CalloutSide::Bottom)
        .elbow()
        .reach(48.0)
        .tone(Tone::Accent)
        .chip(),
    )?;
    vel_callout.show(sc, springs);

    let arrival_1 = s.send(sc, "psy-pkt-1", springs + seconds(0.15), 0.35);
    s.land(sc, "wgpu", arrival_1);
    let arrival_2 = s.send(sc, "psy-pkt-2", c.at("scene plan"), 0.35);
    s.land(sc, "wgpu", arrival_2);

    // "hands it straight to WGPU!" -> Morph from flat 2D plane (0) into a 3D cube (1) and turn!
    let straight = c.at("straight");
    s.morph(sc, "wgpu", straight, 1, 0.9);
    s.ease(
        sc,
        "wgpu.pitch",
        straight,
        std::f32::consts::FRAC_PI_4,
        1.0,
        Ease::Smootherstep,
    );
    s.hit(sc, "wgpu.pulse", straight, 0.85, 0.0);
    let out_arr = s.send(
        sc,
        "psy-out-1",
        out_contact.max(straight + seconds(0.2)),
        0.35,
    );
    s.land(sc, "psy-out", out_arr);
    sfx::WHOOSH.play(sc, "to-wgpu", straight, -13.0);

    // ── Act 3: COMBUSTION ("16-sample temporal motion blur! Depth-of-field bokeh! 3D particle morphs!") ──
    let blur = b.at("motion blur");
    s.bounce(sc, "camera.x", blur, -150.0, 0.24, 0.0);
    s.bounce(sc, "camera.x", blur + seconds(0.32), 0.0, 0.42, 0.22);
    s.hit(sc, "post.zoom", blur, 0.2, 0.0);
    clock.roll(sc, blur, "1.2506")?;
    let s_arr = s.send(sc, "psy-pkt-3", blur, 0.2);
    s.land(sc, "wgpu", s_arr);
    let o_arr = s.send(sc, "psy-out-2", blur + seconds(0.12), 0.2);
    s.land(sc, "psy-out", o_arr);
    sfx::WHOOSH.play(sc, "whip-blur", blur.saturating_sub(seconds(0.04)), -9.0);

    // Depth-of-field bokeh!
    let bokeh = b.at("bokeh");
    s.hit(sc, "post.bloom", bokeh, 1.35, 0.18);
    s.hit(sc, "wgpu.pulse", bokeh, 1.1, 0.0);
    for (ring, delay) in [("ripple-1", 0.0), ("ripple-2", 0.12)] {
        let at = bokeh + seconds(delay);
        s.channel(sc, &format!("{ring}.opacity"), 0.0);
        s.set(sc, &format!("{ring}.opacity"), at, 0.85);
        s.ease(sc, &format!("{ring}.expand"), at, 1.0, 0.85, Ease::CubicOut);
    }
    sfx::BLOOM.play(sc, "bokeh-bloom", bokeh, -9.0);

    // 3D particle morphs! Cube (1) -> Torus (2) -> Sphere (3)
    let morphs = b.at("morphs");
    s.morph(sc, "wgpu", morphs, 2, 0.55);
    s.morph(sc, "wgpu", morphs + seconds(0.6), 3, 0.65);
    s.ease(sc, "wgpu.roll", morphs, 0.8, 1.1, Ease::Smootherstep);

    // "OR VOLUMETRIC RAYMARCHED COMBUSTION WHEN THE SERVER EXPLODES?!!"
    let volumetric = b.at("volumetric");
    sfx::RISER.play(sc, "riser", volumetric.saturating_sub(seconds(0.6)), -11.0);
    s.to(sc, "wgpu.hurt", volumetric, 0.75, 0.7);
    s.ease(sc, "camera.quake", volumetric, 0.9, 0.8, Ease::Linear);

    let explodes = b.at("explodes");
    let burst = explodes.saturating_sub(seconds(0.1));
    cpu_callout.hide(sc, explodes);
    vel_callout.hide(sc, explodes);
    s.to(sc, "wgpu-caption.opacity", explodes, 0.0, 0.2);
    s.clock_for(sc, "wgpu.burst", burst, combustion::DURATION);
    s.to(sc, "wgpu.hurt", explodes, 1.0, 0.15);
    s.jolt(sc, explodes, [0.0, 1.0], 1.0);
    s.set(sc, "camera.quake", explodes, 1.65);
    s.ease(sc, "camera.quake", explodes, 0.0, 1.6, Ease::CubicOut);
    s.hit(sc, "post.exposure", explodes, 2.7, 1.0);
    s.set(sc, "post.flash", explodes, 0.82);
    s.ease(sc, "post.flash", explodes, 0.0, 0.65, Ease::CubicOut);
    s.hit(sc, "post.chroma", explodes, 0.52, 0.0);
    s.hit(sc, "post.zoom", explodes, 0.28, 0.0);
    s.hit(sc, "post.bloom", explodes, 0.65, 0.18);
    sfx::BOOM.play(
        sc,
        "explode-boom",
        explodes.saturating_sub(seconds(0.02)),
        -6.5,
    );
    sfx::DEATH.play(sc, "explode-death", explodes + seconds(0.04), -10.5);

    for (index, card) in ["psy-plan", "psy-out", "ff-scene", "ff-svgr", "ff-libav"]
        .into_iter()
        .enumerate()
    {
        let passes = s.shock_kick(sc, "wgpu", explodes, card, 13.0, None);
        s.set(sc, &format!("{card}.damage"), passes, 1.0);
        s.glitch(sc, card, passes, [7.0 + index as f32, 9.0, 8.0]);
        s.to(sc, &format!("{card}.status"), passes, 1.0, 0.2);
    }
    for beam in ["psy-in", "psy-emit"] {
        s.to(
            sc,
            &format!("{beam}.break"),
            explodes + seconds(0.12),
            1.0,
            0.7,
        );
    }

    // "...try putting that in an SVG path, Dmitriy."
    let try_svg = b.at("try putting");
    let mut svg_callout = CalloutActor::declare(
        sc,
        "svg-callout",
        &CalloutPlan::new(
            CalloutAnchorPlan::Stage {
                id: "wgpu".into(),
                element: "wgpu".into(),
                edge: CalloutSide::Bottom,
                side: None,
            },
            vec![
                span("<path d=\"M 0 0 ", Tone::Muted),
                span("...volumetric fire??", Tone::Error),
                span("\" />", Tone::Muted),
            ],
        )
        .side(CalloutSide::Bottom)
        .elbow()
        .reach(48.0)
        .tone(Tone::Error)
        .chip(),
    )?;
    svg_callout.show(sc, try_svg);
    sfx::TICK.play(sc, "svg-joke", try_svg, -14.0);

    // "And then you rewind the clock, and every spring keeps its velocity."
    let rewind = b.at("rewind").saturating_sub(seconds(0.12));
    svg_callout.hide(sc, rewind);
    s.rewind(sc, rewind, 0.16);
    sfx::LAUNCH.play(
        sc,
        "rewind-sfx",
        rewind.saturating_sub(seconds(0.08)),
        -11.5,
    );
    s.unburst(sc, "wgpu", rewind + seconds(0.08), 1.25);
    s.to(sc, "wgpu-caption.opacity", rewind + seconds(0.5), 1.0, 0.4);
    clock.roll(sc, rewind + seconds(0.2), "0.00")?;

    for (index, card) in ["psy-plan", "psy-out", "ff-scene", "ff-svgr", "ff-libav"]
        .into_iter()
        .enumerate()
    {
        let at = rewind + seconds(0.35 + index as f64 * 0.06);
        s.glitch(sc, card, at + seconds(0.22), [8.0, 9.0, 7.0]);
        s.set(sc, &format!("{card}.damage"), at + seconds(0.3), 0.0);
        let target_status = if card == "psy-plan" { 2.0 } else { 0.0 };
        s.to(
            sc,
            &format!("{card}.status"),
            at + seconds(0.3),
            target_status,
            0.2,
        );
    }
    for beam in ["psy-in", "psy-emit"] {
        s.to(
            sc,
            &format!("{beam}.break"),
            rewind + seconds(0.35),
            0.0,
            0.7,
        );
    }

    // "Good morning, Dmitriy. Welcome to Psychopomp."
    // Fade the diagram cards and center the glowing wgpu sphere with the closing title.
    let outro = b.at("good morning");
    clock.hide(sc, outro);
    for element in [
        "hdr-fframes",
        "hdr-psy",
        "ff-scene",
        "ff-svgr",
        "ff-libav",
        "ff-path",
        "psy-plan",
        "psy-out",
        "psy-in",
        "psy-emit",
        "wgpu-caption",
    ] {
        s.to(sc, &format!("{element}.opacity"), outro, 0.0, 0.55);
    }
    s.to(sc, "wgpu.y", outro, -165.0, 1.1);
    s.to(sc, "wgpu.x", outro, 20.0, 1.1);
    s.to(sc, "camera.z", outro, 0.0, 1.4);
    s.fade_in(sc, "outro-gm", outro + seconds(0.15), 1.0, 0.55);
    s.hit(sc, "wgpu.pulse", outro + seconds(0.2), 0.6, 0.0);
    sfx::SPARKLE.play(sc, "outro-gm-sfx", outro + seconds(0.15), -13.0);

    let welcome = b.at("welcome");
    s.fade_in(sc, "outro-title", welcome, 1.0, 0.5);
    s.type_in(sc, "outro-url", welcome + seconds(0.25), 42.0);
    s.hit(sc, "wgpu.pulse", welcome + seconds(0.15), 0.8, 0.0);
    sfx::BLOOM.play(sc, "welcome-bloom", welcome, -13.0);

    scene.finish().context("architecture")
}

#[cfg(test)]
mod tests {
    use psychopomp::{
        math::{Vec3, vec2},
        stage::Camera,
    };

    #[test]
    fn architecture_cards_fit_working_camera_bounds() {
        for z in [-60.0, 0.0, 40.0] {
            let camera = Camera::at(Vec3::new(0.0, 0.0, z), vec2(1920.0, 1080.0));
            for (at, width) in [
                (super::FF_SCENE, 310.0),
                (super::FF_SVGR, 320.0),
                (super::FF_LIBAV, 310.0),
                (super::PSY_PLAN, 330.0),
                (super::PSY_OUT, 320.0),
            ] {
                let (center, scale) = camera.project(Vec3::from(at)).unwrap();
                let half_w = width * 0.5 * scale;
                assert!(
                    center.x - half_w > 24.0 && center.x + half_w < 1896.0,
                    "{at:?} clips horizontally at z={z}: center.x={}",
                    center.x
                );
            }
        }
    }
}

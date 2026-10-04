//! Psychopomp introduces itself, sweetly and then not at all. A particle orb
//! wakes up, two clients plug into it, packets fly, the orb is blown up and
//! rewound, and a fever-pitch feature list ramps a sustained camera quake
//! until everything whites out into silence and the title.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds, spread},
    callout::{CalloutActor, CalloutAnchorPlan, CalloutPlan, CalloutSide},
    caption::{CaptionAlign, CaptionSpanPlan},
    effects::combustion,
    math::{Vec3, easing::Ease},
    narration::Narration,
    plan::{
        MediaKindPlan, MediaPlan, MediaRolePlan, ReelPlan, ReelSegmentPlan, ReelTransitionStyle,
        ScenePlan,
    },
    rolling::{RollingNumberActor, RollingNumberPlan},
    stage::{StageActor, StageElement, StagePlan, StagePost},
    tone::Tone,
};

const ORB: [f32; 3] = [960.0, 500.0, 0.0];
const TERMINAL: [f32; 3] = [360.0, 500.0, -40.0];
const DESKTOP: [f32; 3] = [1560.0, 500.0, -40.0];
const CARD: [f32; 2] = [300.0, 110.0];
/// The camera leans in on the orb while the narrator whispers.
const LEAN: f32 = 240.0;

/// Packets that fly on "packets fly down the wires": id, beam, label.
const VOLLEY: [(&str, &str, &str); 6] = [
    ("p1", "link-l", "GET"),
    ("p2", "link-r", "POST"),
    ("p3", "link-l", "PUT"),
    ("p4", "link-r", "PATCH"),
    ("p5", "link-l", "HEAD"),
    ("p6", "link-r", "DELETE"),
];

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let narration = Narration::load(&root.join("narration"))?;
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "psychopomp-intro".to_owned(),
        segments: vec![ReelSegmentPlan {
            transition_nanos: 0,
            transition_style: ReelTransitionStyle::Dip,
            transition_focus: None,
            transition_wipe: None,
            plan: film(&narration)?,
        }],
    };
    reel.validate()?;
    let output = root.join("psychopomp-intro.reel.json");
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

fn client(id: &str, at: [f32; 3], title: &str) -> StageElement {
    StageElement::card(id, at, CARD, title)
        .statuses(&[("connected", Tone::Muted), ("disconnected", Tone::Error)])
        .tone(Tone::Request)
}

fn stage_plan() -> StagePlan {
    let mut elements = vec![
        StageElement::orb("orb", ORB, 140.0)
            .points(900)
            .tone(Tone::Plain),
        client("terminal", TERMINAL, "terminal"),
        client("desktop", DESKTOP, "desktop app"),
        StageElement::beam("link-l", "terminal", "orb").tone(Tone::Request),
        StageElement::beam("link-r", "desktop", "orb").tone(Tone::Request),
        StageElement::label(
            "title",
            [960.0, 760.0, 0.0],
            112.0,
            &[("psychopomp", Tone::Accent)],
        ),
        StageElement::label(
            "subtitle",
            [960.0, 845.0, 0.0],
            30.0,
            &[("motion graphics in rust", Tone::Plain)],
        ),
        StageElement::label(
            "url",
            [960.0, 905.0, 0.0],
            28.0,
            &[
                ("github.com/kitlangton/", Tone::Muted),
                ("psychopomp", Tone::Plain),
            ],
        ),
        StageElement::label(
            "shout-frame",
            [960.0, 330.0, 160.0],
            150.0,
            &[("ANY FRAME!", Tone::Plain)],
        ),
        StageElement::label(
            "shout-order",
            [960.0, 690.0, 160.0],
            150.0,
            &[("ANY ORDER!", Tone::Accent)],
        ),
        StageElement::label(
            "diff-old",
            [690.0, 840.0, 0.0],
            30.0,
            &[("- ", Tone::Error), ("yield* server.kill()", Tone::Muted)],
        )
        .align(CaptionAlign::Left),
        StageElement::label(
            "diff-new",
            [690.0, 888.0, 0.0],
            30.0,
            &[
                ("+ ", Tone::Success),
                ("yield* rewind(server)", Tone::Plain),
            ],
        )
        .align(CaptionAlign::Left),
        StageElement::ring("ripple-1", ORB, 150.0)
            .thickness(1.4)
            .tone(Tone::Accent),
        StageElement::ring("ripple-2", ORB, 150.0).thickness(1.4),
        StageElement::ring("calm", ORB, 152.0)
            .thickness(1.4)
            .tone(Tone::Success),
        StageElement::ring("calm-outer", ORB, 158.0)
            .thickness(1.4)
            .tone(Tone::Success),
    ];
    for (id, beam, text) in VOLLEY {
        elements.push(
            StageElement::packet(id, beam)
                .labeled(text)
                .tone(Tone::Request),
        );
    }
    elements.extend([
        StageElement::packet("whip-l", "link-l").tone(Tone::Accent),
        StageElement::packet("whip-r", "link-r").tone(Tone::Accent),
        StageElement::packet("flood-1", "link-l"),
        StageElement::packet("flood-2", "link-r"),
        StageElement::packet("flood-3", "link-l").tone(Tone::Accent),
        StageElement::packet("flood-4", "link-r").tone(Tone::Accent),
    ]);
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

/// Glitch layouts about a frame and a half apart, then still.
fn glitch(s: &mut StageActor, sc: &mut PlanBuilder, card: &str, at: u64, seeds: [f32; 3]) {
    let mut step = at;
    for seed in seeds.into_iter().chain([0.0]) {
        s.set(sc, &format!("{card}.glitch"), step, seed);
        step += seconds(0.027);
    }
}

struct Sfx(&'static str, f64);
const TICK: Sfx = Sfx("visual-effects/task-running.wav", 0.13);
const IMPACT: Sfx = Sfx("opencode-hot-reload/impact.wav", 0.51);
const LAUNCH: Sfx = Sfx("opencode-hot-reload/launch.wav", 1.36);
const DEATH: Sfx = Sfx("visual-effects/task-death.wav", 1.09);
const GLITCH: Sfx = Sfx("pr-walkthrough/glitch.wav", 0.2);
const CONFIRM: Sfx = Sfx("opencode-hot-reload/confirm.wav", 0.34);
const BLOOM: Sfx = Sfx("effect-shows-errors/prismatic-bloom.wav", 0.785);
const RISER: Sfx = Sfx("psychopomp-intro/riser.wav", 5.66);
const BOOM: Sfx = Sfx("psychopomp-intro/boom.wav", 2.99);
const WHOOSH: Sfx = Sfx("psychopomp-intro/whoosh.wav", 0.61);
const SPARKLE: Sfx = Sfx("psychopomp-intro/sparkle.wav", 1.47);

fn sound(sc: &mut PlanBuilder, id: &str, Sfx(file, length): Sfx, at: u64, gain_db: f32) {
    let length = seconds(length);
    sc.media(MediaPlan {
        id: id.to_owned(),
        path: PathBuf::from(format!("../../assets/{file}")),
        kind: MediaKindPlan::Audio,
        role: MediaRolePlan::Layer,
        source_start_nanos: 0,
        source_end_nanos: length,
        timeline_start_nanos: at,
        timeline_end_nanos: at + length,
        gain_db,
    });
}

fn film(narration: &Narration) -> Result<ScenePlan> {
    let gap = seconds(0.35);
    let reading = narration.reading(
        seconds(1.3),
        [("hush", gap), ("toys", gap), ("fever", seconds(2.6))],
    )?;
    let mut scene = PlanBuilder::new("psychopomp-intro", reading.duration());
    let [hush, toys, fever] = reading.place(&mut scene);
    let h = |phrase: &str| hush.at(phrase);
    let t = |phrase: &str| toys.at(phrase);
    let f = |phrase: &str| fever.at(phrase);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    for (property, initial) in [
        ("camera.z", -180.0),
        ("camera.dof", 0.45),
        ("orb.scale", 0.5),
        ("orb.blur", 12.0),
        ("orb.rotation", -2.0),
        ("post.vignette", 0.22),
        ("post.exposure", 1.0),
        ("post.bloom", 0.18),
        ("title.scale", 0.92),
        ("shout-frame.scale", 1.7),
        ("shout-order.scale", 1.7),
    ] {
        s.channel(sc, property, initial);
    }

    // ── Hush: a soul wakes up in the dark ──
    s.to(sc, "camera.z", 0, 0.0, 2.6);
    s.bounce(sc, "orb.scale", seconds(0.2), 1.0, 1.1, 0.2);
    s.to(sc, "orb.blur", seconds(0.2), 0.0, 0.9);
    s.ease(sc, "orb.rotation", seconds(0.2), 0.0, 1.6, Ease::CubicOut);
    s.fade_in(sc, "orb", seconds(0.2), 1.0, 0.8);
    sound(sc, "wake", SPARKLE, seconds(0.25), -16.0);
    s.hit(sc, "orb.pulse", h("hi"), 0.4, 0.0);
    s.fade_in(sc, "title", h("this is"), 1.0, 0.6);
    s.bounce(sc, "title.scale", h("this is"), 1.0, 1.0, 0.15);
    sound(sc, "title", SPARKLE, h("this is"), -14.0);
    s.type_in(sc, "subtitle", h("tiny"), 34.0);

    // "Every frame is a pure function of time": a clock that can be anywhere.
    let mut clock = RollingNumberActor::declare(
        sc,
        "clock",
        RollingNumberPlan::new([960.0, 190.0], 34.0, "0.00")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .prefix(vec![span("t = ", Tone::Muted)])
            .suffix(vec![span(" s", Tone::Muted)])
            .chip(),
    )?;
    clock.show(sc, h("every frame"));
    clock.roll(sc, h("pure function"), "1.25")?;
    clock.roll(sc, h("time"), "2.50")?;

    // ANY FRAME! ANY ORDER! The title gets out of the way of the shouting.
    for name in ["title", "subtitle"] {
        s.to(sc, &format!("{name}.opacity"), h("any frame"), 0.0, 0.12);
    }
    for (shout, at, direction, value) in [
        ("shout-frame", h("any frame"), [1.0, 0.3], "31.40"),
        ("shout-order", h("any order"), [-1.0, -0.4], "0.07"),
    ] {
        s.channel(sc, &format!("{shout}.opacity"), 0.0);
        s.set(sc, &format!("{shout}.opacity"), at, 1.0);
        s.bounce(sc, &format!("{shout}.scale"), at, 1.0, 0.3, 0.3);
        s.jolt(sc, at, direction, 1.0);
        s.hit(sc, "post.chroma", at, 0.35, 0.0);
        s.hit(sc, "post.zoom", at, 0.14, 0.0);
        s.hit(sc, "post.exposure", at, 1.7, 1.0);
        s.set(sc, "post.flash", at, 0.4);
        s.ease(sc, "post.flash", at, 0.0, 0.3, Ease::CubicOut);
        s.hit(sc, "orb.pulse", at, 1.0, 0.0);
        clock.roll(sc, at, value)?;
        sound(sc, &format!("{shout}-impact"), IMPACT, at, -6.0);
        sound(
            sc,
            &format!("{shout}-whoosh"),
            WHOOSH,
            at.saturating_sub(seconds(0.12)),
            -12.0,
        );
    }
    s.ease(
        sc,
        "orb.rotation",
        h("any order"),
        -1.2,
        0.5,
        Ease::CubicOut,
    );

    // ...isn't that nice? Everything settles at once.
    let nice = h("nice");
    for shout in ["shout-frame", "shout-order"] {
        s.to(sc, &format!("{shout}.opacity"), nice, 0.0, 0.35);
        s.to(sc, &format!("{shout}.scale"), nice, 0.9, 0.5);
    }
    clock.roll(sc, nice, "0.00")?;
    for name in ["title", "subtitle"] {
        s.to(
            sc,
            &format!("{name}.opacity"),
            nice + seconds(0.45),
            1.0,
            0.6,
        );
    }
    s.hit(sc, "orb.pulse", nice + seconds(0.1), 0.35, 0.0);
    sound(sc, "nice", SPARKLE, nice, -15.0);

    // ── Toys: cards, wires, packets ──
    let cards = t("cards");
    clock.hide(sc, cards);
    for name in ["title", "subtitle"] {
        s.to(sc, &format!("{name}.opacity"), cards, 0.0, 0.5);
    }
    s.to(sc, "camera.z", cards, -60.0, 1.8);
    for (card, at) in [("terminal", t("settle")), ("desktop", t("softly"))] {
        s.settle_in(sc, card, at);
        sound(sc, &format!("settle-{card}"), TICK, at, -18.0);
    }
    for (link, at) in [
        ("link-l", t("wires draw")),
        ("link-r", t("draw themselves")),
    ] {
        let contact = s.connect(sc, link, at, 0.7);
        sound(sc, &format!("connect-{link}"), CONFIRM, contact, -18.0);
    }

    // AND PACKETS FLY DOWN THE WIRES!!
    let packets = t("packets");
    let wires = toys.at_after("wires", "packets");
    s.to(sc, "camera.quake", packets, 0.7, 0.25);
    s.to(sc, "camera.z", packets, 30.0, 0.6);
    for ((id, beam, _), launch) in VOLLEY
        .iter()
        .zip(spread(VOLLEY.len() as u64, packets, wires))
    {
        let arrival = s.send(sc, id, launch, 0.32);
        let card = if *beam == "link-l" {
            "terminal"
        } else {
            "desktop"
        };
        s.hit(sc, &format!("{card}.flash"), launch, 0.5, 0.0);
        s.land(sc, "orb", arrival);
        let push = if *beam == "link-l" { 1.0 } else { -1.0 };
        s.jolt(sc, arrival, [push, 0.2], 0.5);
        sound(
            sc,
            &format!("{id}-send"),
            WHOOSH,
            launch.saturating_sub(seconds(0.05)),
            -16.0,
        );
        sound(sc, &format!("{id}-hit"), IMPACT, arrival, -15.0);
    }
    s.hit(sc, "post.zoom", wires, 0.12, 0.0);

    // ...and when a server misbehaves (whispered): lean in, close and red.
    let when = t("and when");
    s.to(sc, "camera.quake", when, 0.0, 0.5);
    s.to(sc, "camera.z", when, LEAN, 2.6);
    s.to(sc, "camera.focus", when, 0.0, 1.0);
    s.to(sc, "post.vignette", when, 0.62, 1.6);
    s.to(sc, "orb.hurt", t("misbehaves"), 0.7, 1.2);
    for card in ["terminal", "desktop"] {
        s.to(sc, &format!("{card}.dim"), when, 0.5, 1.0);
    }

    // YOU BLOW IT UP!!!
    let up = t("up");
    let burst = up.saturating_sub(seconds(0.12));
    s.clock_for(sc, "orb.burst", burst, combustion::DURATION);
    s.to(sc, "orb.hurt", up, 1.0, 0.2);
    s.jolt(sc, up, [0.0, 1.0], 1.0);
    s.set(sc, "camera.quake", up, 1.6);
    s.ease(sc, "camera.quake", up, 0.0, 1.7, Ease::CubicOut);
    s.hit(sc, "post.exposure", up, 2.6, 1.0);
    s.set(sc, "post.flash", up, 0.8);
    s.ease(sc, "post.flash", up, 0.0, 0.7, Ease::CubicOut);
    s.hit(sc, "post.chroma", up, 0.5, 0.0);
    s.hit(sc, "post.zoom", up, 0.28, 0.0);
    s.hit(sc, "post.bloom", up, 0.6, 0.18);
    s.to(sc, "post.vignette", up, 0.3, 0.6);
    s.to(sc, "camera.z", up, -90.0, 1.1);
    sound(
        sc,
        "blow-boom",
        BOOM,
        up.saturating_sub(seconds(0.02)),
        -7.0,
    );
    sound(sc, "blow-death", DEATH, up + seconds(0.04), -11.0);
    for (index, (card, link, at)) in [
        ("terminal", "link-l", TERMINAL),
        ("desktop", "link-r", DESKTOP),
    ]
    .into_iter()
    .enumerate()
    {
        let away = Vec3::from(at) - Vec3::from(ORB);
        let push = away.truncate().normalize() * 14.0;
        let passes = up
            + seconds(f64::from(combustion::shock_arrival(
                away.truncate().length(),
            )));
        s.kick(
            sc,
            [&format!("{card}.x"), &format!("{card}.y")],
            passes,
            push.into(),
        );
        s.to(
            sc,
            &format!("{link}.break"),
            up + seconds(0.1 + index as f64 * 0.08),
            1.0,
            0.8,
        );
        s.set(sc, &format!("{card}.damage"), passes, 1.0);
        glitch(s, sc, card, passes, [7.0, 9.0, 8.0]);
        s.to(sc, &format!("{card}.status"), passes, 1.0, 0.2);
        sound(sc, &format!("glitch-{card}"), GLITCH, passes, -18.0);
    }

    // ...and then you rewind it. Like nothing happened.
    let rewind = t("rewind").saturating_sub(seconds(0.15));
    s.clock_for(sc, "post.rewind", rewind, 1.4);
    s.hit(sc, "post.chroma", rewind, 0.15, 0.0);
    sound(
        sc,
        "rewind",
        LAUNCH,
        rewind.saturating_sub(seconds(0.1)),
        -12.0,
    );
    s.ease(
        sc,
        "orb.burst",
        rewind + seconds(0.1),
        0.0,
        1.3,
        Ease::Smootherstep,
    );
    s.set(sc, "orb.burst", rewind + seconds(1.4), -1.0);
    s.to(sc, "orb.hurt", rewind + seconds(0.6), 0.0, 0.6);
    s.to(sc, "camera.z", rewind, -40.0, 1.8);
    s.to(sc, "post.vignette", rewind, 0.22, 1.0);
    for (index, (card, link)) in [("terminal", "link-l"), ("desktop", "link-r")]
        .into_iter()
        .enumerate()
    {
        let at = rewind + seconds(0.4 + index as f64 * 0.1);
        s.to(sc, &format!("{link}.break"), at, 0.0, 0.8);
        glitch(s, sc, card, at + seconds(0.3), [8.0, 9.0, 7.0]);
        s.set(sc, &format!("{card}.damage"), at + seconds(0.38), 0.0);
        s.to(sc, &format!("{card}.status"), at + seconds(0.38), 0.0, 0.2);
        s.to(sc, &format!("{card}.dim"), at, 0.0, 0.6);
    }
    let nothing = t("nothing happened");
    s.hit(sc, "orb.pulse", nothing, 0.4, 0.0);
    sound(sc, "nothing", SPARKLE, nothing, -15.0);

    // ── Fever: every feature at once, and the floor starts shaking ──
    // The peak lands on the shouted "code", not on "just".
    let code = fever.at_after("code", "just");
    sound(
        sc,
        "riser",
        RISER,
        code.saturating_sub(seconds(5.66)),
        -11.0,
    );
    let rolling = f("rolling numbers");
    s.ease(sc, "camera.quake", rolling, 0.45, 2.4, Ease::Linear);
    s.ease(sc, "camera.quake", f("motion blur"), 1.0, 1.6, Ease::Linear);
    s.ease(
        sc,
        "camera.quake",
        f("screen shake"),
        1.5,
        1.0,
        Ease::Linear,
    );
    s.ease(sc, "camera.quake", f("just"), 2.0, 0.4, Ease::Linear);
    s.ease(
        sc,
        "orb.rotation",
        rolling,
        -1.2 + 9.0,
        6.2,
        Ease::Decelerate(2.0),
    );
    s.to(sc, "camera.z", rolling, 20.0, 4.0);

    let mut counter = RollingNumberActor::declare(
        sc,
        "counter",
        RollingNumberPlan::new([960.0, 175.0], 58.0, "0")
            .aligned(CaptionAlign::Center)
            .tone(Tone::Accent)
            .suffix(vec![span(" frames", Tone::Muted)]),
    )?;
    counter.show(sc, rolling);
    for (phrase, value) in [
        ("numbers", "1,337"),
        ("callouts", "88,888"),
        ("code diffs", "1,000,000"),
        ("motion blur", "9,999,999"),
        ("bloom", "42,000,000"),
        ("screen shake", "999,999,999"),
    ] {
        counter.roll(sc, f(phrase), value)?;
    }

    // Callouts!
    let mut soul = CalloutActor::declare(
        sc,
        "soul",
        &CalloutPlan::new(
            CalloutAnchorPlan::Stage {
                id: "orb".into(),
                element: "orb".into(),
                edge: CalloutSide::TopRight,
                side: None,
            },
            vec![
                span("a soul, ", Tone::Plain),
                span("probably", Tone::Accent),
            ],
        )
        .side(CalloutSide::TopRight)
        .elbow()
        .reach(70.0)
        .tone(Tone::Accent)
        .chip(),
    )?;
    let mut pinned = CalloutActor::declare(
        sc,
        "pinned",
        &CalloutPlan::new(
            CalloutAnchorPlan::Stage {
                id: "terminal".into(),
                element: "terminal".into(),
                edge: CalloutSide::Bottom,
                side: None,
            },
            vec![span("pinned to anything", Tone::Plain)],
        )
        .side(CalloutSide::BottomRight)
        .elbow()
        .reach(60.0)
        .tone(Tone::Success)
        .chip(),
    )?;
    let callouts = f("callouts");
    soul.show(sc, callouts);
    pinned.show(sc, callouts + seconds(0.18));
    sound(sc, "callouts", CONFIRM, callouts, -13.0);

    // Code diffs!
    let diffs = f("code diffs");
    s.type_in(sc, "diff-old", diffs, 90.0);
    s.type_in(sc, "diff-new", diffs + seconds(0.22), 90.0);
    sound(sc, "diffs", TICK, diffs, -12.0);

    // Motion blur! A whip pan while two packets tear down the wires.
    let blur = f("motion blur");
    s.bounce(sc, "camera.x", blur, -170.0, 0.25, 0.0);
    s.bounce(sc, "camera.x", f("bloom") - seconds(0.25), 0.0, 0.45, 0.25);
    for (id, offset) in [("whip-l", 0.0), ("whip-r", 0.12)] {
        let launch = blur + seconds(offset);
        let arrival = s.send(sc, id, launch, 0.16);
        s.land(sc, "orb", arrival);
    }
    s.hit(sc, "post.zoom", blur, 0.22, 0.0);
    sound(
        sc,
        "blur-whoosh",
        WHOOSH,
        blur.saturating_sub(seconds(0.05)),
        -9.0,
    );

    // Bloom! Light floods out of the orb in two ripples.
    let bloom = f("bloom");
    s.hit(sc, "post.bloom", bloom, 1.4, 0.18);
    s.hit(sc, "orb.pulse", bloom, 1.2, 0.0);
    for (ring, delay) in [("ripple-1", 0.0), ("ripple-2", 0.12)] {
        let at = bloom + seconds(delay);
        s.channel(sc, &format!("{ring}.opacity"), 0.0);
        s.set(sc, &format!("{ring}.opacity"), at, 0.8);
        s.ease(sc, &format!("{ring}.expand"), at, 1.0, 0.9, Ease::CubicOut);
    }
    sound(sc, "bloom", BLOOM, bloom, -8.0);

    // SCREEN SHAKE!!
    let shake = f("screen shake");
    s.jolt(sc, shake, [0.7, 1.0], 1.0);
    s.hit(sc, "post.chroma", shake, 0.35, 0.0);
    sound(sc, "shake", IMPACT, shake, -5.0);
    for (card, seed) in [("terminal", 5.0), ("desktop", 6.0)] {
        glitch(s, sc, card, shake, [seed, 9.0, 7.0]);
    }

    // IT'S ALL JUST CODE!!! Flood every wire, then white out.
    let just = f("just");
    for (index, id) in ["flood-1", "flood-2", "flood-3", "flood-4"]
        .iter()
        .enumerate()
    {
        let arrival = s.send(sc, id, just + seconds(0.09 * index as f64), 0.22);
        s.land(sc, "orb", arrival);
    }
    for link in ["link-l", "link-r"] {
        s.to(sc, &format!("{link}.flow"), just, 1.0, 0.3);
        s.hit(sc, &format!("{link}.surge"), code, 1.0, 0.0);
    }
    soul.emphasize(sc, code);
    pinned.emphasize(sc, code);
    s.jolt(sc, code, [-0.5, 1.0], 1.0);
    s.hit(sc, "post.chroma", code, 0.6, 0.0);
    s.hit(sc, "post.zoom", code, 0.34, 0.0);
    s.hit(sc, "post.exposure", code, 3.4, 1.0);
    // A full whiteout that hides the cut to silence, then a long cooling tail.
    s.set(sc, "post.flash", code, 1.0);
    s.ease(
        sc,
        "post.flash",
        code + seconds(0.12),
        0.0,
        1.5,
        Ease::CubicOut,
    );
    s.hit(sc, "post.bloom", code, 1.2, 0.18);
    for (card, seed) in [("terminal", 9.0), ("desktop", 8.0)] {
        glitch(s, sc, card, code, [seed, 7.0, 9.0]);
    }
    sound(
        sc,
        "code-boom",
        BOOM,
        code.saturating_sub(seconds(0.02)),
        -6.0,
    );
    sound(sc, "code-impact", IMPACT, code, -10.0);

    // Silence. The whiteout hides the cut; only the orb is left breathing.
    let cut = code + seconds(0.32);
    s.set(sc, "camera.quake", cut, 0.0);
    s.ease(sc, "orb.rotation", cut, -1.2 + 9.6, 3.0, Ease::CubicOut);
    for link in ["link-l", "link-r"] {
        s.to(sc, &format!("{link}.flow"), cut, 0.0, 0.4);
        s.to(sc, &format!("{link}.opacity"), cut, 0.0, 0.5);
    }
    for name in ["terminal", "desktop", "diff-old", "diff-new"] {
        s.to(sc, &format!("{name}.opacity"), cut, 0.0, 0.5);
    }
    counter.hide(sc, cut);
    soul.hide(sc, cut);
    pinned.hide(sc, cut);
    s.to(sc, "camera.z", cut, -30.0, 2.4);
    s.to(sc, "camera.x", cut, 0.0, 1.2);

    // Psychopomp. Give it to your agent. It knows what to do.
    let name = f("psychopomp");
    s.set(sc, "title.scale", name, 0.86);
    s.bounce(sc, "title.scale", name, 1.0, 0.9, 0.2);
    s.to(sc, "title.opacity", name, 1.0, 0.5);
    s.hit(sc, "orb.pulse", name, 0.5, 0.0);
    sound(sc, "name", SPARKLE, name, -12.0);
    s.type_in(sc, "url", f("give it"), 42.0);
    let knows = f("knows");
    s.fade_in(sc, "calm", knows, 0.35, 0.3);
    s.fade_in(sc, "calm-outer", knows + seconds(0.06), 0.5, 0.3);
    s.hit(sc, "orb.pulse", knows, 0.7, 0.0);
    sound(sc, "knows", BLOOM, knows, -14.0);

    scene.finish().context("psychopomp-intro")
}

#[cfg(test)]
mod tests {
    use psychopomp::{
        math::{Vec3, vec2},
        stage::Camera,
    };

    #[test]
    fn clients_and_title_fit_the_working_cameras() {
        for z in [-180.0, -90.0, -60.0, -40.0, 0.0, 30.0] {
            let camera = Camera {
                position: Vec3::new(0.0, 0.0, z),
                size: vec2(1920.0, 1080.0),
            };
            for at in [super::TERMINAL, super::DESKTOP] {
                let (center, scale) = camera.project(Vec3::from(at)).unwrap();
                let half = vec2(super::CARD[0], super::CARD[1]) * (0.5 * scale);
                assert!(
                    center.x - half.x > 40.0 && center.x + half.x < 1880.0,
                    "{at:?} clips at z {z}"
                );
            }
        }
    }
}

//! Saved settings cross a schema gate on the way into the app. In rc.117 the
//! gate strips any field the schema does not name: the unknown field strikes
//! it, falls away, and the next save leaves an empty red slot. After the fix
//! the legacy read shape has an open rest, the lower gate opens, and both
//! fields make the round trip home.
use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    caption::CaptionAlign,
    math::easing::Ease,
    narration::Narration,
    plan::ScenePlan,
    sfx,
    stage::{DRAW_CURVE, StageActor, StageElement, StagePlan, StagePost},
    tone::Tone,
};

use crate::{DROP, chip, footer, header, span};

const SAVED_X: f32 = 380.0;
const APP_X: f32 = 1540.0;
const GATE_X: f32 = 960.0;
const THEME_Y: f32 = 600.0;
const FLAG_Y: f32 = 690.0;
const FIELD_SIZE: [f32; 2] = [380.0, 64.0];
const CODE_X: f32 = 540.0;
/// Field cards float just in front of the gate they cross.
const FIELD_Z: f32 = -4.0;
/// Where the unknown field's right edge meets the gate.
const STRIKE: f32 = GATE_X - FIELD_SIZE[0] * 0.5 - SAVED_X;
/// How far a field travels from saved settings to app state.
const ACROSS: f32 = APP_X - SAVED_X;
/// Gravity for a falling field: slow off the edge, accelerating down.
const FALL: Ease = Ease::CubicBezier([0.55, 0.0, 1.0, 0.45]);

fn code(id: &str, y: f32, parts: &[(&str, Tone)]) -> StageElement {
    StageElement::label(id, [CODE_X, y, -2.0], 24.0, parts).align(CaptionAlign::Left)
}

fn ring(id: &str, y: f32) -> StageElement {
    StageElement::ring(id, [GATE_X, y, 0.0], 5.0).thickness(2.0)
}

fn stage_plan() -> StagePlan {
    let removed = |tone| {
        [
            ("  { onExcessProperty: ", Tone::Muted),
            ("\"preserve\"", tone),
            (" })", Tone::Muted),
        ]
    };
    let elements = vec![
        StageElement::card("parser", [960.0, 330.0, 0.0], [900.0, 200.0], " "),
        StageElement::label(
            "parser-tab",
            [516.0, 208.0, 0.0],
            18.0,
            &[
                ("read schema", Tone::Plain),
                (" · condensed from persistence/schema.ts", Tone::Muted),
            ],
        )
        .align(CaptionAlign::Left),
        code(
            "code-read",
            305.0,
            &[(
                "SchemaParser.decodeUnknownResult(definition.read,",
                Tone::Plain,
            )],
        ),
        code("code-preserve", 350.0, &removed(Tone::Warning)),
        code("code-preserve-red", 350.0, &removed(Tone::Error)),
        code(
            "code-legacy",
            285.0,
            &[
                ("const legacy = (fields) => ", Tone::Muted),
                ("Schema.StructWithRest(", Tone::Accent),
            ],
        ),
        code(
            "code-known",
            330.0,
            &[("  Schema.Struct(fields),", Tone::Plain)],
        ),
        code(
            "code-rest",
            375.0,
            &[
                ("  [", Tone::Muted),
                (
                    "Schema.Record(Schema.String, Schema.Unknown)",
                    Tone::Success,
                ),
                ("])", Tone::Muted),
            ],
        ),
        StageElement::label(
            "col-saved",
            [SAVED_X, 528.0, 0.0],
            20.0,
            &[("saved settings", Tone::Muted)],
        ),
        StageElement::label(
            "col-app",
            [APP_X, 528.0, 0.0],
            20.0,
            &[("app state", Tone::Muted)],
        ),
        ring("gate-mid", 645.0),
        ring("gate-end", 780.0),
        StageElement::beam("gate-top", "parser", "gate-mid"),
        StageElement::beam("gate-bottom", "gate-mid", "gate-end"),
        // The empty slot the stripped field leaves; only its red ghost draws.
        StageElement::card("flag-slot", [SAVED_X, FLAG_Y, 0.0], FIELD_SIZE, " "),
        StageElement::card(
            "theme",
            [SAVED_X, THEME_Y, FIELD_Z],
            FIELD_SIZE,
            "\"theme\": \"dark\"",
        ),
        StageElement::card(
            "flag",
            [SAVED_X, FLAG_Y, FIELD_Z],
            FIELD_SIZE,
            "\"experimentalFlag\": true",
        )
        .tone(Tone::Accent),
    ];
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

/// Glide a field along x; returns when it lands.
fn travel(s: &mut StageActor, sc: &mut PlanBuilder, field: &str, at: u64, to: f32) -> u64 {
    s.ease(sc, &format!("{field}.x"), at, to, 1.0, Ease::Smootherstep);
    at + seconds(1.0)
}

/// A field crossing the upper gate halfway through its glide lights it locally.
fn cross(s: &mut StageActor, sc: &mut PlanBuilder, at: u64, id: &str) {
    let crossing = at + seconds(0.5);
    s.hit(sc, "gate-top.surge", crossing, 0.35, 0.0);
    sfx::TICK.play(sc, id, crossing, -26.0);
}

pub fn build(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(seconds(0.5), [("settings", seconds(0.7))])?;
    let mut scene = PlanBuilder::new("settings", reading.duration());
    let [spoken] = reading.place(&mut scene);
    let w = |phrase: &str| spoken.at(phrase);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    header(sc, "1 · saved settings", Some(seconds(0.15)))?;
    let mut before_chip = chip(sc, "chip-before", Tone::Muted, "before")?;
    before_chip.show(sc, seconds(0.4));
    s.channel(sc, "flag-slot.opacity", 0.0);
    s.channel(sc, "camera.z", -60.0);
    s.to(sc, "camera.z", 0, 0.0, 1.6);

    // Saved settings: one field the schema knows, one it does not.
    let saved = w("saved settings");
    s.fade_in(sc, "col-saved", saved, 1.0, 0.4);
    s.settle_in(sc, "theme", saved + seconds(0.08));
    s.settle_in(sc, "flag", saved + seconds(0.2));

    // The parser and its gate.
    let parser = w("the new parser");
    s.settle_in(sc, "parser", parser.saturating_sub(seconds(0.25)));
    s.fade_in(sc, "parser-tab", parser, 1.0, 0.4);
    s.type_in(sc, "code-read", parser, 95.0);
    s.type_in(sc, "code-preserve", parser + seconds(0.4), 95.0);
    for (index, ring) in ["gate-mid", "gate-end"].iter().enumerate() {
        s.fade_in(
            sc,
            ring,
            parser + seconds(0.4 + index as f64 * 0.3),
            0.8,
            0.3,
        );
    }
    let top = s.connect(sc, "gate-top", parser + seconds(0.3), 0.4);
    let bottom = s.connect(sc, "gate-bottom", top, 0.25);
    for gate in ["gate-top", "gate-bottom"] {
        s.to(sc, &format!("{gate}.flow"), bottom + seconds(0.5), 0.0, 0.4);
    }
    s.fade_in(sc, "col-app", parser + seconds(0.6), 1.0, 0.4);

    // rc.117 removed the option that kept undeclared keys.
    let drops = w("drops");
    s.ease(
        sc,
        "code-preserve.opacity",
        drops,
        0.0,
        0.12,
        Ease::Smootherstep,
    );
    s.channel(sc, "code-preserve-red.opacity", 0.0);
    s.ease(
        sc,
        "code-preserve-red.opacity",
        drops,
        1.0,
        0.12,
        Ease::Smootherstep,
    );
    sfx::GLITCH.play(sc, "preserve-red", drops, -24.0);
    let gone = drops + seconds(0.5);
    s.ease(sc, "code-preserve-red.y", gone, 22.0, 0.45, FALL);
    s.ease(
        sc,
        "code-preserve-red.opacity",
        gone,
        0.0,
        0.4,
        Ease::Smootherstep,
    );

    // Read into the app: the known field crosses, the unknown one strikes the
    // lower gate, is stripped, and falls away.
    let read = w("so an unknown");
    let theme_in = travel(s, sc, "theme", read, ACROSS);
    cross(s, sc, read, "theme-cross");
    s.hit(sc, "theme.flash", theme_in, 0.45, 0.0);
    sfx::SEND.play(sc, "theme-send", read, -18.0);
    let launch = w("unknown setting").saturating_sub(seconds(0.25));
    s.ease(
        sc,
        "flag.x",
        launch,
        STRIKE,
        0.55,
        Ease::CubicBezier([0.5, 0.0, 1.0, 1.0]),
    );
    let strike = launch + seconds(0.55);
    s.bounce(sc, "flag.x", strike, STRIKE - 18.0, 0.5, 0.3);
    s.hit(sc, "gate-bottom.surge", strike, 0.6, 0.0);
    s.twang(sc, "gate-bottom", strike);
    s.hit(sc, "flag.alarm", strike, 1.0, 0.6);
    for (step, seed) in [7.0, 9.0, 8.0, 0.0].into_iter().enumerate() {
        s.set(
            sc,
            "flag.glitch",
            strike + seconds(0.027 * step as f64),
            seed,
        );
    }
    s.jolt(sc, strike, [1.0, 0.0], 0.22);
    sfx::FAILURE.play(sc, "strike", strike, -14.0);
    let vanish = w("vanish");
    s.ease(sc, "flag.y", vanish, 320.0, 0.75, FALL);
    s.ease(
        sc,
        "flag.opacity",
        vanish + seconds(0.15),
        0.0,
        0.6,
        Ease::Smootherstep,
    );
    DROP.play(sc, "drop", vanish, -15.0);

    // The next save writes back only what survived.
    let save = w("on the next save");
    travel(s, sc, "theme", save, 0.0);
    cross(s, sc, save, "theme-save");
    s.hit(sc, "theme.flash", save + seconds(1.0), 0.4, 0.0);
    let empty = save + seconds(0.75);
    s.to(sc, "flag-slot.ghost", empty, 0.7, 0.3);
    sfx::GLITCH.play(sc, "empty-slot", empty, -22.0);
    let mut footer_before = footer(
        sc,
        "footer-before",
        vec![
            span("rc.117 strips undeclared keys: ", Tone::Plain),
            span("experimentalFlag", Tone::Error),
            span(" is lost", Tone::Plain),
        ],
    )?;
    footer_before.type_in(sc, vanish + seconds(0.2), 52.0, 0.5);

    // The fix: the unknown field returns to its slot, and the camera reads the
    // new legacy read shape.
    let switch = w("now every legacy").saturating_sub(seconds(0.15));
    before_chip.hide(sc, switch);
    footer_before.hide(sc, switch);
    let mut after_chip = chip(sc, "chip-after", Tone::Success, "after the fix")?;
    after_chip.show(sc, switch + seconds(0.25));
    for name in ["code-read", "code-preserve"] {
        s.to(sc, &format!("{name}.opacity"), switch, 0.0, 0.25);
    }
    s.to(sc, "flag-slot.ghost", switch, 0.0, 0.3);
    s.to(sc, "flag.alarm", switch, 0.0, 0.4);
    for (axis, seconds) in [("x", 0.75), ("y", 0.7)] {
        s.ease(
            sc,
            &format!("flag.{axis}"),
            switch,
            0.0,
            seconds,
            Ease::Smootherstep,
        );
    }
    s.ease(sc, "flag.opacity", switch, 1.0, 0.3, Ease::Smootherstep);
    s.hit(sc, "flag.flash", switch + seconds(0.75), 0.35, 0.0);
    sfx::TICK.play(sc, "flag-return", switch + seconds(0.75), -24.0);
    s.to(sc, "camera.y", switch + seconds(0.1), -200.0, 1.2);
    s.to(sc, "camera.z", switch + seconds(0.1), 260.0, 1.2);
    s.type_in(sc, "code-legacy", w("legacy shape"), 72.0);

    // An open rest: pull back as the lower gate retracts into the joint.
    let open = w("open rest");
    s.to(
        sc,
        "camera.y",
        open.saturating_sub(seconds(0.2)),
        -40.0,
        1.4,
    );
    s.to(sc, "camera.z", open.saturating_sub(seconds(0.2)), 50.0, 1.4);
    let gap = open + seconds(0.3);
    s.ease(sc, "gate-bottom.draw", gap, 0.0, 0.45, DRAW_CURVE);
    s.to(sc, "gate-end.opacity", gap + seconds(0.3), 0.0, 0.3);
    s.hit(sc, "gate-mid.scale", gap + seconds(0.45), 1.6, 1.0);
    sfx::TICK.play(sc, "gate-open", gap + seconds(0.45), -22.0);

    // The known fields, plus a catch-all record: both cross.
    let known = w("the known fields");
    s.type_in(sc, "code-known", known, 60.0);
    let theme_pass = known + seconds(0.1);
    let theme_in = travel(s, sc, "theme", theme_pass, ACROSS);
    cross(s, sc, theme_pass, "theme-pass");
    s.hit(sc, "theme.flash", theme_in, 0.45, 0.0);
    s.type_in(sc, "code-rest", w("plus a"), 72.0);
    let rest = w("catch").saturating_sub(seconds(0.1));
    let flag_in = travel(s, sc, "flag", rest, ACROSS);
    s.hit(sc, "gate-mid.scale", rest + seconds(0.5), 1.8, 1.0);
    s.hit(sc, "flag.glow", flag_in, 0.8, 0.0);
    s.hit(sc, "flag.flash", flag_in, 0.5, 0.0);
    sfx::SEND.play(sc, "flag-pass", rest, -18.0);

    // Saved again: both come home to exactly their slots.
    let back = w("come back").saturating_sub(seconds(0.3));
    for (index, field) in ["theme", "flag"].iter().enumerate() {
        let at = back + seconds(0.12 * index as f64);
        let home = travel(s, sc, field, at, 0.0);
        s.hit(sc, &format!("{field}.flash"), home, 0.55, 0.0);
        sfx::TICK.play(sc, format!("{field}-home"), home, -22.0);
    }
    cross(s, sc, back, "theme-back");
    let mut footer_after = footer(
        sc,
        "footer-after",
        vec![
            span("known fields + a catch-all rest: ", Tone::Plain),
            span("nothing is dropped", Tone::Success),
        ],
    )?;
    footer_after.type_in(sc, w("exactly"), 55.0, 0.6);
    scene.finish().context("settings")
}

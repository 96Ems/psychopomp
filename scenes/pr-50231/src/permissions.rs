//! The climax. An agent's permission config wires into a stack of rules that
//! are evaluated last match wins, from the bottom up. rc.117 decodes the keys in
//! schema order: two rule cards trade places, the wires cross, and a probe for
//! `shell` and one for `edit` both land on `* allow`. Rewind; `inInputOrder`
//! rebuilds the stack in the order written; the same probes land on `shell ask`
//! and `edit deny`. The camera then flies into `inInputOrder`'s code.
use anyhow::{Context, Result};
use kinograph::{
    author::PlanBuilder,
    caption::CaptionAlign,
    editor::{
        EditorInlineRevealPlan, EditorPartPlan, EditorRecipePlan, EditorSemanticRangePlan,
        LineMarkPlan,
    },
    highlight,
    math::{Vec2, Vec3, easing::Ease, vec2},
    plan::{ScenePlan, SpringPlan, destination_channel},
    stage::{Camera, DRAW_CURVE, StageActor, StagePlan},
    tone::Tone,
};

use crate::{
    FAILURE, MARK, POST, RESOLUTION, REWIND, SEND, SHUFFLE, ZOOM, beam, card, chip,
    diff::{Diff, fresh, keep},
    footer, header, label,
    narration::Narration,
    ns, packet, sound, span, status,
};

const SLOTS: [f32; 3] = [420.0, 530.0, 640.0];
const CONFIG_X: f32 = 400.0;
const RULES_X: f32 = 1060.0;
const RULE_SIZE: [f32; 2] = [360.0, 92.0];
/// Rules in the order the author wrote them, top to bottom.
const RULES: [(&str, &str, &str, Tone); 3] = [
    ("star", "*", "allow", Tone::Success),
    ("shell", "shell", "ask", Tone::Warning),
    ("edit", "edit", "deny", Tone::Error),
];
const CONFIG: [&str; 3] = [
    "\"*\": \"allow\"",
    "\"bash\": \"ask\"",
    "\"edit\": \"deny\"",
];
const FIX: [f32; 3] = [730.0, 262.0, 0.0];
const FIX_SIZE: [f32; 2] = [300.0, 64.0];
/// Probes wait below the stack and rise beside it: (card, x).
const PROBE_Y: f32 = 830.0;
const PROBES: [(&str, f32); 2] = [("shell", 1380.0), ("edit", 1590.0)];
/// Probes stop just above (shell) or below (edit) a slot's center, so the
/// edit probe's wire passes beneath the shell probe when both hit one card.
const PROBE_OFFSET: [f32; 2] = [-28.0, 28.0];
/// Where the camera rests before flying into `inInputOrder`.
const CLOSING_CAMERA: [f32; 3] = [-60.0, -60.0, 60.0];
/// The editor line of `migrateAgent`'s call site.
const CALL_LINE: &str = "line-11";
/// How far rc.117's schema order moves `*` down and `edit` up.
const SWAP: f32 = SLOTS[2] - SLOTS[0];

fn stage_plan() -> StagePlan {
    let mut elements = vec![
        label(
            "config-head",
            [CONFIG_X, 350.0, 0.0],
            20.0,
            CaptionAlign::Center,
            &[("agent config · as written", Tone::Muted)],
        ),
        label(
            "rules-head",
            [RULES_X, 350.0, 0.0],
            20.0,
            CaptionAlign::Center,
            &[("rules · ", Tone::Muted), ("last match wins", Tone::Plain)],
        ),
        label(
            "scan",
            [RULES_X, 724.0, 0.0],
            18.0,
            CaptionAlign::Center,
            &[
                ("↑ ", Tone::Accent),
                ("checked from the bottom", Tone::Muted),
            ],
        ),
        card(
            "fix",
            FIX,
            FIX_SIZE,
            "inInputOrder",
            Vec::new(),
            Tone::Accent,
        ),
    ];
    for (index, ((id, title, action, tone), config)) in RULES.iter().zip(CONFIG).enumerate() {
        elements.extend([
            card(
                &format!("config-{index}"),
                [CONFIG_X, SLOTS[index], 0.0],
                [330.0, 78.0],
                config,
                Vec::new(),
                Tone::Plain,
            ),
            card(
                &format!("rule-{id}"),
                [RULES_X, SLOTS[index], 0.0],
                RULE_SIZE,
                title,
                vec![status(action, *tone)],
                *tone,
            ),
            beam(
                &format!("wire-{index}"),
                &format!("config-{index}"),
                &format!("rule-{id}"),
                Tone::Plain,
            ),
            packet(
                &format!("key-{index}"),
                &format!("wire-{index}"),
                Tone::Request,
            ),
        ]);
    }
    for (name, x) in PROBES {
        elements.push(card(
            &format!("probe-{name}"),
            [x, PROBE_Y, -4.0],
            [150.0, 56.0],
            name,
            Vec::new(),
            Tone::Request,
        ));
    }
    for (id, probe, rule, tone) in [
        ("miss-shell", "shell", "star", Tone::Error),
        ("miss-edit", "edit", "star", Tone::Error),
        ("match-shell", "shell", "shell", Tone::Warning),
        ("match-edit", "edit", "edit", Tone::Error),
    ] {
        elements.push(beam(
            id,
            &format!("probe-{probe}"),
            &format!("rule-{rule}"),
            tone,
        ));
    }
    // Verdicts sit under each probe's resting place.
    for (id, x, y, action, tone, sign) in [
        (
            "verdict-shell-before",
            1380.0,
            668.0,
            "allow",
            Tone::Error,
            " ✗",
        ),
        (
            "verdict-edit-before",
            1590.0,
            724.0,
            "allow",
            Tone::Error,
            " ✗",
        ),
        (
            "verdict-shell-after",
            1380.0,
            558.0,
            "ask",
            Tone::Warning,
            " ✓",
        ),
        (
            "verdict-edit-after",
            1590.0,
            724.0,
            "deny",
            Tone::Error,
            " ✓",
        ),
    ] {
        let mark = if sign == " ✓" {
            Tone::Success
        } else {
            Tone::Error
        };
        elements.push(label(
            id,
            [x, y, 0.0],
            22.0,
            CaptionAlign::Center,
            &[("→ ", Tone::Muted), (action, tone), (sign, mark)],
        ));
    }
    StagePlan {
        post: POST,
        elements,
    }
}

/// `inInputOrder`'s card on screen once the closing camera settles.
fn fix_rect() -> [f32; 4] {
    let camera = Camera {
        position: Vec3::from(CLOSING_CAMERA),
        size: vec2(1920.0, 1080.0),
    };
    let (center, scale) = camera
        .project(Vec3::from(FIX))
        .expect("the fix card is in front of the camera");
    let size = Vec2::from(FIX_SIZE) * scale;
    let corner = center - size * 0.5;
    [corner.x, corner.y, size.x, size.y]
}

/// Swap `*` and `edit`: into rc.117's schema order, or back to the written
/// order. Like shuffling a deck: `*` swings out in front of the stack while
/// `edit` dips behind it, so the `shell` card between them occludes one and is
/// cleared by the other.
fn shuffle(s: &mut StageActor, sc: &mut PlanBuilder, at: u64, into_schema_order: bool) -> u64 {
    let sign = if into_schema_order { 1.0 } else { 0.0 };
    for (card, dy, arc, depth) in [("star", SWAP, 400.0, -60.0), ("edit", -SWAP, -230.0, 240.0)] {
        s.ease(
            sc,
            &format!("rule-{card}.y"),
            at,
            dy * sign,
            1.0,
            Ease::Smootherstep,
        );
        for (axis, peak) in [("x", arc), ("z", depth)] {
            let channel = format!("rule-{card}.{axis}");
            s.ease(sc, &channel, at, peak, 0.5, Ease::Smootherstep);
            s.ease(sc, &channel, at + ns(0.5), 0.0, 0.5, Ease::Smootherstep);
        }
    }
    sc.media(sound(
        &format!("shuffle-{}", if into_schema_order { "in" } else { "back" }),
        SHUFFLE,
        at,
        -15.0,
    ));
    let landed = at + ns(1.0);
    for index in 0..RULES.len() {
        s.twang(sc, &format!("wire-{index}"), landed);
    }
    for card in ["star", "edit"] {
        s.hit(sc, &format!("rule-{card}.flash"), landed, 0.35, 0.0);
    }
    s.jolt(sc, landed, [0.0, 1.0], 0.12);
    landed
}

/// Raise a probe from below the stack to beside `slot`.
fn rise(s: &mut StageActor, sc: &mut PlanBuilder, probe: usize, slot: f32, at: u64, seconds: f64) {
    let name = PROBES[probe].0;
    let target = slot + PROBE_OFFSET[probe] - PROBE_Y;
    s.ease(
        sc,
        &format!("probe-{name}.y"),
        at,
        target,
        seconds as f32,
        Ease::Smootherstep,
    );
}

/// The probe's wire draws to the card it matched; returns the contact time.
fn strike(s: &mut StageActor, sc: &mut PlanBuilder, wire: &str, contact: u64) -> u64 {
    s.connect(sc, wire, contact.saturating_sub(ns(0.52)), 0.22)
}

pub fn build(narration: &Narration) -> Result<(ScenePlan, ScenePlan, [f32; 4])> {
    let before_clip = narration.clip("permissions-before")?;
    let after_clip = narration.clip("permissions-after")?;
    // Split the after clip in the pause before "And the new tests" (measured
    // silence 5.88–6.14 s): the code segment carries the rest, entered by a
    // zoom into `inInputOrder`.
    let split = after_clip.split(6.0, "denied", "new tests");
    let lead = ns(0.5);
    let rewind = ns(1.9);
    let hold = ns(0.15);
    let after_start = lead + before_clip.duration() + rewind;
    let zoom_start = after_start + split + hold;
    let mut scene = PlanBuilder::new("permissions", zoom_start + ZOOM);
    let before = before_clip.place(&mut scene, lead);
    let after = after_clip.place_range(&mut scene, after_start, 0, split, "-stage");
    let b = |phrase: &str| before.at(phrase);
    let a = |phrase: &str| after.at(phrase);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    header(sc, "3 · permission order", Some(ns(0.15)))?;
    let mut before_chip = chip(sc, "chip-before", Tone::Muted, "before")?;
    before_chip.show(sc, ns(0.4));
    s.channel(sc, "camera.z", -80.0);
    s.to(sc, "camera.z", 0, 0.0, 1.8);

    // The config wires into its rules, in the order written.
    let rules = b("permission rules");
    s.to(sc, "config-head.opacity", rules, 1.0, 0.4);
    s.to(sc, "rules-head.opacity", rules + ns(0.3), 1.0, 0.4);
    for (index, (id, ..)) in RULES.iter().enumerate() {
        let stagger = ns(0.1 * index as f64);
        s.settle_in(sc, &format!("config-{index}"), rules + stagger);
        s.settle_in(sc, &format!("rule-{id}"), rules + ns(0.3) + stagger);
        let wire = format!("wire-{index}");
        let contact = s.connect(sc, &wire, rules + ns(0.6) + ns(0.12 * index as f64), 0.45);
        s.to(sc, &format!("{wire}.flow"), contact + ns(0.7), 0.0, 0.45);
    }
    // Last match wins: the check runs bottom to top.
    let wins = b("last match wins");
    s.type_in(sc, "scan", wins, 60.0);
    for (step, (id, ..)) in RULES.iter().rev().enumerate() {
        s.hit(
            sc,
            &format!("rule-{id}.flash"),
            wins + ns(0.65 + 0.14 * step as f64),
            0.35,
            0.0,
        );
    }

    // rc.117 no longer keeps key order: `*` drops to the bottom, the wires cross.
    shuffle(s, sc, b("keeps your key order"), true);

    // Each key travels its (now crossed) wire to its rule.
    for (index, phrase) in ["allows everything", "asks before shell", "denies edits"]
        .iter()
        .enumerate()
    {
        let at = b(phrase);
        let arrival = s.send(sc, &format!("key-{index}"), at, 0.6);
        s.hit(sc, &format!("config-{index}.flash"), at, 0.35, 0.0);
        s.hit(
            sc,
            &format!("rule-{}.flash", RULES[index].0),
            arrival,
            0.5,
            0.0,
        );
        sc.media(sound(&format!("key-{index}"), SEND, at, -21.0));
    }

    // Both probes rise from the bottom and match `*` first: silently allowed.
    let would = b("it would have");
    let silently = b("silently");
    let both = b("both");
    for (probe, at) in [(0, would), (1, would + ns(0.15))] {
        s.settle_in(
            sc,
            &format!("probe-{}", PROBES[probe].0),
            at.saturating_sub(ns(0.45)),
        );
    }
    rise(s, sc, 0, SLOTS[2], would, 0.55);
    rise(s, sc, 1, SLOTS[2], both.saturating_sub(ns(1.05)), 0.55);
    for (probe, wire, at) in [
        (0, "miss-shell", silently + ns(0.2)),
        (1, "miss-edit", both),
    ] {
        let name = PROBES[probe].0;
        let contact = strike(s, sc, wire, at);
        s.hit(sc, "rule-star.alarm", contact, 1.4, 0.8);
        let seeds = if probe == 0 {
            [7.0, 9.0, 8.0]
        } else {
            [8.0, 7.0, 9.0]
        };
        for (step, seed) in seeds.into_iter().chain([0.0]).enumerate() {
            s.set(
                sc,
                "rule-star.glitch",
                contact + ns(0.027 * step as f64),
                seed,
            );
        }
        s.jolt(sc, contact, [-1.0, 0.0], 0.18 + 0.12 * probe as f32);
        s.hit(sc, &format!("probe-{name}.alarm"), contact, 1.0, 0.5);
        s.hit(sc, "post.chroma", contact, 0.06, 0.0);
        s.type_in(sc, &format!("verdict-{name}-before"), contact, 40.0);
        sc.media(sound(&format!("miss-{name}"), FAILURE, contact, -13.0));
    }
    let mut footer_before = footer(
        sc,
        "footer-before",
        vec![
            span("keys in schema order: ", Tone::Plain),
            span("both silently allowed", Tone::Error),
        ],
    )?;
    footer_before.type_in(sc, both + ns(0.25), 55.0, 0.5);

    // Rewind to before the probes rose.
    let switch = before.end() + ns(0.3);
    s.channel(sc, "post.rewind", -1.0);
    s.set(sc, "post.rewind", switch, 0.0);
    s.ease(sc, "post.rewind", switch, 1.4, 1.4, Ease::Linear);
    s.hit(sc, "post.chroma", switch, 0.1, 0.0);
    sc.media(sound(
        "rewind",
        REWIND,
        switch.saturating_sub(ns(0.05)),
        -13.0,
    ));
    before_chip.hide(sc, switch);
    footer_before.hide(sc, switch);
    let mut rewind_chip = chip(sc, "chip-rewind", Tone::Accent, "◀◀ rewind")?;
    rewind_chip.show(sc, switch + ns(0.25));
    rewind_chip.hide(sc, switch + ns(1.45));
    let mut after_chip = chip(sc, "chip-after", Tone::Success, "after the fix")?;
    after_chip.show(sc, switch + ns(1.6));
    for (name, _) in PROBES {
        s.to(
            sc,
            &format!("verdict-{name}-before.opacity"),
            switch,
            0.0,
            0.3,
        );
        s.ease(
            sc,
            &format!("probe-{name}.y"),
            switch + ns(0.1),
            0.0,
            1.1,
            Ease::Smootherstep,
        );
        s.to(
            sc,
            &format!("probe-{name}.alarm"),
            switch + ns(0.2),
            0.0,
            0.5,
        );
        s.ease(
            sc,
            &format!("probe-{name}.opacity"),
            switch + ns(0.9),
            0.0,
            0.3,
            Ease::Smootherstep,
        );
    }
    for wire in ["miss-shell", "miss-edit"] {
        s.ease(
            sc,
            &format!("{wire}.draw"),
            switch + ns(0.1),
            0.0,
            0.5,
            DRAW_CURVE,
        );
        s.ease(
            sc,
            &format!("{wire}.port"),
            switch + ns(0.5),
            0.0,
            0.3,
            Ease::Smootherstep,
        );
    }
    s.to(sc, "rule-star.alarm", switch + ns(0.2), 0.0, 0.5);

    // The fix rebuilds the rules in the order written: the wires uncross.
    let fix = a("the fix");
    s.settle_in(sc, "fix", fix.saturating_sub(ns(0.1)));
    let rebuild = a("rebuilds") + ns(0.15);
    s.hit(sc, "fix.glow", rebuild, 0.8, 0.25);
    s.hit(sc, "fix.flash", rebuild, 0.5, 0.0);
    shuffle(s, sc, rebuild, false);
    let mut footer_after = footer(
        sc,
        "footer-after",
        vec![
            span("inInputOrder: rules in ", Tone::Plain),
            span("the order you wrote", Tone::Success),
        ],
    )?;
    footer_after.type_in(sc, rebuild + ns(0.4), 55.0, 0.5);

    // Shell: the probe passes `edit deny` and stops at `shell ask`.
    let asks = a("shell asks");
    s.settle_in(sc, "probe-shell", asks.saturating_sub(ns(0.95)));
    let rise_at = asks.saturating_sub(ns(0.55));
    rise(s, sc, 0, SLOTS[1], rise_at, 0.7);
    s.hit(sc, "rule-edit.flash", rise_at + ns(0.38), 0.2, 0.0);
    let contact = strike(s, sc, "match-shell", a("asks"));
    s.hit(sc, "rule-shell.flash", contact, 0.6, 0.0);
    s.hit(sc, "rule-shell.glow", contact, 0.6, 0.15);
    s.hit(sc, "probe-shell.flash", contact, 0.5, 0.0);
    s.type_in(sc, "verdict-shell-after", contact, 40.0);
    sc.media(sound("match-shell", MARK, contact, -17.0));

    // Edit: the bottom card is `edit deny` again.
    let edits = a("edits are");
    s.settle_in(sc, "probe-edit", edits.saturating_sub(ns(0.7)));
    rise(s, sc, 1, SLOTS[2], edits.saturating_sub(ns(0.3)), 0.5);
    let contact = strike(s, sc, "match-edit", a("denied"));
    s.hit(sc, "rule-edit.flash", contact, 0.6, 0.0);
    s.hit(sc, "rule-edit.glow", contact, 0.6, 0.15);
    s.hit(sc, "probe-edit.flash", contact, 0.5, 0.0);
    s.type_in(sc, "verdict-edit-after", contact, 40.0);
    sc.media(sound("match-edit", RESOLUTION, contact, -16.0));

    // Settle on the fix; the reel's zoom flies into its code.
    for (axis, value) in ["camera.x", "camera.y", "camera.z"]
        .iter()
        .zip(CLOSING_CAMERA)
    {
        s.to(sc, axis, edits, value, 1.4);
    }
    s.to(sc, "fix.glow", contact, 0.6, 0.8);
    after_chip.hide(sc, zoom_start);
    footer_after.hide(sc, zoom_start);
    let stage = scene.finish().context("permissions")?;
    let code = code(after_clip, split)?;
    Ok((stage, code, fix_rect()))
}

/// The change, opened from the `inInputOrder` card. Continues the narration
/// from the split, with the new function whole and the call site swapping.
fn code(clip: &crate::narration::Clip, split: u64) -> Result<ScenePlan> {
    let rest = clip.duration() - split;
    let duration = rest + ns(3.0);
    let mut scene = PlanBuilder::new("permissions-code", duration);
    let spoken = clip.place_range(&mut scene, 0, split, clip.duration(), "-code");
    header(&mut scene, "3 · permission order", None)?;
    let mut change = chip(&mut scene, "chip-change", Tone::Accent, "the change")?;
    change.show(&mut scene, ns(0.5));
    let diff = Diff {
        file_name: "v1/config/migrate.ts",
        lines: vec![
            fresh("// Effect emits declared keys in schema order. Permission rules"),
            fresh("// are last-match-wins, so restore the author's key order."),
            fresh("function inInputOrder(info: ConfigPermissionV1.Info | undefined,"),
            fresh("  input: unknown) {"),
            fresh("  if (info === undefined || typeof input !== \"object\" || input === null)"),
            fresh("    return info"),
            fresh("  return Object.fromEntries(Object.keys(input).flatMap((key) =>"),
            fresh("    Object.hasOwn(info, key) ? [[key, info[key]]] : []))"),
            fresh("}"),
            keep(""),
            keep("// … in migrateAgent(info, permission?: unknown):"),
            // Rewritten in place by `swap_call_site`.
            keep("  permissions: permissions(info.permission),"),
        ],
    };
    diff.declare(&mut scene)?;
    let mut caption = footer(
        &mut scene,
        "footer",
        vec![
            span("condensed · ", Tone::Muted),
            span("new tests", Tone::Accent),
            span(
                " for legacy agent, mode, and markdown agents fail without it",
                Tone::Muted,
            ),
        ],
    )?;
    caption.type_in(&mut scene, spoken.at("new tests"), 60.0, 0.6);
    let at = spoken.at("fail without");
    swap_call_site(scene.finish().context("permissions-code")?, at)
}

/// The call site keeps its identity: only `inInputOrder(` and `, permission)`
/// open inside the stable line, which turns green as they arrive.
fn swap_call_site(mut plan: ScenePlan, at: u64) -> Result<ScenePlan> {
    let actor = plan
        .actors
        .iter_mut()
        .find(|actor| actor.id == "editor")
        .context("code editor")?;
    let mut recipe: EditorRecipePlan = serde_json::from_value(actor.data.clone())?;
    let part = |id: &str, text: &str| EditorPartPlan {
        id: id.into(),
        spans: highlight::typescript(text),
    };
    let line = recipe
        .lines
        .iter_mut()
        .find(|line| line.id == CALL_LINE)
        .context("call site line")?;
    line.parts = vec![
        part("head", "  permissions: permissions("),
        part("wrap", "inInputOrder("),
        part("info", "info.permission"),
        part("input", ", permission)"),
        part("tail", "),"),
    ];
    line.mark = Some(LineMarkPlan::Added);
    for range in ["wrap", "input"] {
        line.semantic_ranges.push(EditorSemanticRangePlan {
            id: range.into(),
            first_part_id: range.into(),
            last_part_id: range.into(),
        });
        recipe
            .additional_inline_reveals
            .push(EditorInlineRevealPlan {
                line_id: CALL_LINE.into(),
                range_id: range.into(),
                channel: Some("call-swap".into()),
                reversed: false,
            });
    }
    recipe.compile()?;
    actor.data = serde_json::to_value(recipe)?;
    for property in ["call-swap", &format!("mark.{CALL_LINE}")] {
        plan.continuous_channels.push(destination_channel(
            "editor",
            property,
            0.0,
            [(at, 1.0)],
            |_, _| SpringPlan::visual(0.4, 0.0),
        ));
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use kinograph::{
        math::{Vec3, vec2},
        stage::{Camera, StageElement},
    };

    #[test]
    fn cards_fit_every_camera_composition() {
        for position in [[0.0, 0.0, -80.0], [0.0, 0.0, 0.0], super::CLOSING_CAMERA] {
            let camera = Camera {
                position: Vec3::from(position),
                size: vec2(1920.0, 1080.0),
            };
            for element in super::stage_plan().elements {
                if let StageElement::Card { id, at, size, .. } = element {
                    let (center, scale) = camera.project(Vec3::from(at)).unwrap();
                    let half = vec2(size[0], size[1]) * (0.5 * scale);
                    let (low, high) = (center - half, center + half);
                    assert!(
                        low.x > 60.0 && high.x < 1860.0 && low.y > 140.0 && high.y < 970.0,
                        "{id} clips at camera {position:?}: {low:?}..{high:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_call_site_swap_has_no_stability_warnings() {
        use kinograph::{editor::inspect_steps, plan::PresentationStepPlan};
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let narration = crate::narration::Narration::load(&root.join("narration")).unwrap();
        let (_, mut code, _) = super::build(&narration).unwrap();
        let end = code.duration_nanos;
        code.presentation_steps = [(0, 1), (end - 2, end - 1)]
            .into_iter()
            .enumerate()
            .map(|(i, (start, hold))| PresentationStepPlan {
                id: format!("step-{i}"),
                title: format!("Step {i}"),
                start_nanos: start,
                hold_nanos: hold,
            })
            .collect();
        let inspection = serde_json::to_value(inspect_steps(&code).unwrap()).unwrap();
        assert_eq!(inspection["warnings"], serde_json::json!([]));
    }

    #[test]
    fn the_zoom_rectangle_matches_the_fix_card() {
        let rect = super::fix_rect();
        assert!(
            rect[2] > super::FIX_SIZE[0],
            "the closing camera magnifies it"
        );
        assert!(rect[0] > 0.0 && rect[1] > 0.0 && rect[0] + rect[2] < 1920.0);
    }
}

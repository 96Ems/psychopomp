//! Four-step port of opencode-architecture's Merge.tsx, not its goo alternative.
//! The diagram recipe knows boxes/wires; this Scene Program knows daemon history.
use anyhow::Result;
use psychopomp::{
    author::{ActorHandle, PlanBuilder, SECOND},
    component_prototype::{
        DIAGRAM, DiagramAnchor, DiagramDelay, DiagramLink, DiagramNode, DiagramPlan, DiagramView,
        Side,
    },
    plan::{DeckPlan, ScenePlan, SlidePlan},
};

const BEAT: u64 = 3 * SECOND;
const CAPTIONS: [&str; 4] = [
    "v1 · one server per client",
    "v1 · same app, twice → two processes",
    "v1 · 3 clients → 3 processes",
    "v2 · 1 shared daemon",
];
const NAMES: [&str; 3] = ["TUI 1", "TUI 2", "DESKTOP"];
const WIDTH_SEED: f32 = 8. / 300.;

fn x(index: usize, step: usize) -> f32 {
    // Hidden nodes already occupy their first visible position. Retained nodes
    // recenter as requested by the reference; new pairs need not slide in sideways.
    let count = (step + 1).min(3).max(index + 1);
    960. + (index as f32 - (count - 1) as f32 / 2.) * 400.
}

fn delay(delays: &mut Vec<DiagramDelay>, property: String, from: f32, to: f32, millis: u64) {
    delays.push(DiagramDelay {
        property,
        from,
        to,
        millis,
    });
}

#[allow(clippy::too_many_arguments)]
fn track(
    p: &mut PlanBuilder,
    actor: &ActorHandle,
    property: &str,
    values: [f32; 4],
    normal: [f32; 2],
    merge: [f32; 2],
    delays: &[DiagramDelay],
) {
    let c = p.continuous(actor, property, values[0]);
    for (i, pair) in values.windows(2).enumerate() {
        if pair[0] == pair[1] {
            continue;
        }
        let wait = delays
            .iter()
            .find(|d| d.property == property && d.from == pair[0] && d.to == pair[1])
            .map_or(0, |d| d.millis * 1_000_000);
        let [duration, bounce] = if i == 2 { merge } else { normal };
        p.spring(&c, (i + 1) as u64 * BEAT + wait, pair[1], duration, bounce);
    }
}

pub fn build_scene() -> Result<ScenePlan> {
    build_view(DiagramView::Flat)
}

pub fn build_deck() -> Result<DeckPlan> {
    let deck = DeckPlan {
        version: 1,
        id: "daemon-views".into(),
        slides: vec![
            SlidePlan {
                title: "Daemon · flat".into(),
                plan: build_scene()?,
            },
            SlidePlan {
                title: "Daemon · isometric".into(),
                plan: build_view(DiagramView::Isometric)?,
            },
        ],
    };
    deck.validate()?;
    Ok(deck)
}

fn build_view(view: DiagramView) -> Result<ScenePlan> {
    let isometric = view == DiagramView::Isometric;
    let entrance_blur = if isometric { 6. } else { 12. };
    let focus_duration = if isometric { 0.12 } else { 0.14 };
    let mut p = PlanBuilder::new("daemon-merge", 4 * BEAT);
    for (i, caption) in CAPTIONS.iter().enumerate() {
        let start = i as u64 * BEAT;
        p.presentation_step(
            format!("step-{i}"),
            *caption,
            start,
            if i == 0 { 0 } else { start + 2 * SECOND },
        );
        p.cue(format!("step-{i}"), start, start + BEAT);
    }
    let mut nodes = Vec::new();
    let mut links = Vec::new();
    let mut delays = Vec::new();
    for (i, name) in NAMES.iter().enumerate() {
        let client = format!("client-{i}");
        let server = format!("server-{i}");
        nodes.push(DiagramNode {
            id: client.clone(),
            center: [x(i, 0), 440.],
            size: [300., 112.],
            label: (*name).into(),
            alternate_label: None,
            muted: false,
        });
        nodes.push(DiagramNode {
            id: server.clone(),
            center: [x(i, 0), 658.],
            size: [300., 92.],
            label: "server".into(),
            alternate_label: (i == 1).then(|| "daemon".into()),
            muted: true,
        });
        links.push(DiagramLink {
            id: format!("wire-{i}"),
            from: DiagramAnchor {
                node: client,
                side: Side::Bottom,
                offset: 0.,
            },
            to: DiagramAnchor {
                node: server.clone(),
                side: Side::Top,
                offset: 0.,
            },
        });
        let mut entrances = vec![("opacity", 0., 1.), ("blur", entrance_blur, 0.)];
        if isometric {
            entrances.push(("depth", 8., 48.));
            entrances.push(("width-reveal", WIDTH_SEED, 1.));
        } else {
            entrances.push(("scale", 1.08, 1.));
        }
        for (property, from, to) in entrances {
            delay(
                &mut delays,
                format!("node.{server}.{property}"),
                from,
                to,
                120,
            );
        }
        delay(&mut delays, format!("link.wire-{i}.draw"), 0., 1., 60);
        if i != 1 {
            delay(&mut delays, format!("node.{server}.shell"), 1., 0., 100);
            delay(&mut delays, format!("node.{server}.ink"), 1., 0., 80);
        }
    }
    // Flat paints the retained server last. Isometric sorts sampled solid depth
    // first, and uses this authored order only to break coincident-depth ties.
    nodes.sort_by_key(|node| usize::from(node.id == "server-1"));
    delay(&mut delays, "node.server-1.glow".into(), 0., 1., 220);
    let actor = p.actor(
        "diagram",
        DIAGRAM,
        DiagramPlan {
            view,
            bounds: [80., 152., 1760., 776.],
            title: "00A · DAEMON / MERGE".into(),
            captions: CAPTIONS.map(str::to_owned).to_vec(),
            nodes,
            links,
            delays: delays.clone(),
        },
    )?;
    let mut animate = |property: &str, values: [f32; 4], normal: [f32; 2], merged: [f32; 2]| {
        track(&mut p, &actor, property, values, normal, merged, &delays)
    };
    for i in 0..3 {
        let presence = std::array::from_fn(|step| if step >= i { 1. } else { 0. });
        for kind in ["client", "server"] {
            let prefix = format!("node.{kind}-{i}");
            let server = kind == "server";
            animate(
                &format!("{prefix}.x"),
                std::array::from_fn(|step| {
                    if server && step == 3 {
                        960.
                    } else {
                        x(i, step)
                    }
                }),
                [0.45, 0.],
                [0.32, 0.],
            );
            animate(&format!("{prefix}.opacity"), presence, [0.1, 0.], [0.1, 0.]);
            animate(
                &format!("{prefix}.scale"),
                presence.map(|a| if isometric { 1. } else { 1.08 - 0.08 * a }),
                [0.3, 0.1],
                [0.3, 0.1],
            );
            animate(
                &format!("{prefix}.blur"),
                presence.map(|a| entrance_blur * (1. - a)),
                [focus_duration, 0.],
                [focus_duration, 0.],
            );
            if isometric {
                // One direction from a fixed base: a quick, critically damped
                // upward extrusion. No falling lift to fight it or bounce.
                animate(
                    &format!("{prefix}.depth"),
                    presence.map(|a| 8. + 40. * a),
                    [0.22, 0.],
                    [0.22, 0.],
                );
                // Let adjacent retained boxes make room. Width opens slightly
                // behind depth, without another wait or whole-label scaling.
                // A separate presence multiplier also keeps the server wait
                // correct when skipping directly to its wider merge destination.
                animate(
                    &format!("{prefix}.width-reveal"),
                    presence.map(|a| WIDTH_SEED + (1. - WIDTH_SEED) * a),
                    [0.34, 0.],
                    [0.34, 0.],
                );
            }
            if server {
                animate(
                    &format!("{prefix}.width"),
                    [300., 300., 300., 380.],
                    [0.32, 0.],
                    [0.32, 0.],
                );
                if i == 1 {
                    animate(
                        &format!("{prefix}.label"),
                        [0., 0., 0., 1.],
                        [0.32, 0.],
                        [0.32, 0.],
                    );
                    animate(
                        &format!("{prefix}.emphasis"),
                        [0., 0., 0., 1.],
                        [0.25, 0.],
                        [0.25, 0.],
                    );
                    animate(
                        &format!("{prefix}.glow"),
                        [0., 0., 0., 1.],
                        [0.45, 0.],
                        [0.45, 0.],
                    );
                } else {
                    animate(
                        &format!("{prefix}.shell"),
                        [1., 1., 1., 0.],
                        [0.22, 0.],
                        [0.22, 0.],
                    );
                    animate(
                        &format!("{prefix}.ink"),
                        [1., 1., 1., 0.],
                        [0.22, 0.],
                        [0.22, 0.],
                    );
                }
            }
        }
        // A fast spring replaces the reference's 400ms ease-out draw; it keeps
        // the head/edge attached and redirectable instead of replaying a CSS path.
        animate(
            &format!("link.wire-{i}.draw"),
            presence,
            [0.2, 0.],
            [0.2, 0.],
        );
        animate(
            &format!("link.wire-{i}.opacity"),
            presence,
            [0.1, 0.],
            [0.1, 0.],
        );
        animate(
            &format!("link.wire-{i}.emphasis"),
            [0., 0., 0., 1.],
            [0.25, 0.],
            [0.25, 0.],
        );
        animate(
            &format!("link.wire-{i}.to-offset"),
            [
                0.,
                0.,
                0.,
                if isometric { 0. } else { (i as f32 - 1.) * 88. },
            ],
            [0.45, 0.],
            [0.32, 0.],
        );
    }
    let mut plan = p.finish()?;
    if view == DiagramView::Isometric {
        plan.id = "daemon-isometric".into();
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isometric_entrance_is_a_fast_critical_rise_from_a_fixed_base() {
        use psychopomp::{
            motion::{MotionState, Spring},
            plan::{ScalarPlan, TrackEventPlan},
        };
        let plan = build_view(DiagramView::Isometric).unwrap();
        assert!(
            plan.continuous_channels
                .iter()
                .all(|c| !c.property.ends_with(".lift")),
            "a descending lift must not fight the upward extrusion"
        );
        for kind in ["client", "server"] {
            let property = format!("node.{kind}-1.depth");
            let channel = plan
                .continuous_channels
                .iter()
                .find(|c| c.property == property)
                .unwrap();
            let TrackEventPlan::Spring {
                response_seconds,
                damping_ratio,
                at_nanos,
                target: ScalarPlan::Literal(target),
                ..
            } = channel.events[0]
            else {
                panic!("expected spring")
            };
            assert_eq!(damping_ratio, 1., "entrance must be critically damped");
            assert!(
                response_seconds <= 0.2641,
                "entrance must be <=220ms visual duration"
            );
            assert_eq!(
                at_nanos,
                BEAT + if kind == "server" { 120_000_000 } else { 0 }
            );
            let ScalarPlan::Literal(initial) = channel.initial else {
                panic!("expected literal depth")
            };
            assert_eq!((initial, target), (8., 48.));
            let spring = Spring::new(response_seconds, damping_ratio);
            let mut previous = initial;
            for millis in 0..1000 {
                let state = spring.sample(MotionState::at(initial), target, millis as f32 / 1000.);
                assert!(state.position >= previous && state.position <= target);
                assert!(state.velocity >= 0., "top face must only rise from rest");
                previous = state.position;
            }
            assert!(
                spring
                    .sample(MotionState::at(initial), target, 0.22)
                    .position
                    > 46.5
            );
        }
    }

    #[test]
    fn views_share_identity_and_layout_but_isometric_has_volume_motion() {
        let deck = build_deck().unwrap();
        let a = &deck.slides[0].plan;
        let b = &deck.slides[1].plan;
        let common = |plan: &ScenePlan| {
            serde_json::to_value(
                plan.continuous_channels
                    .iter()
                    .filter(|c| {
                        ![
                            ".scale",
                            ".blur",
                            ".depth",
                            ".lift",
                            ".width-reveal",
                            ".to-offset",
                        ]
                        .iter()
                        .any(|suffix| c.property.ends_with(suffix))
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        };
        assert_eq!(common(a), common(b));
        assert!(
            b.continuous_channels
                .iter()
                .any(|c| c.property == "node.client-1.depth")
        );
        assert!(
            b.continuous_channels
                .iter()
                .all(|c| !c.property.ends_with(".lift"))
        );
        assert!(
            a.continuous_channels
                .iter()
                .all(|c| !c.property.starts_with("indicator.") && c.property != "caption")
        );
        let a: DiagramPlan = serde_json::from_value(a.actors[0].data.clone()).unwrap();
        let b: DiagramPlan = serde_json::from_value(b.actors[0].data.clone()).unwrap();
        for plan in [&a, &b] {
            assert_eq!(
                plan.nodes
                    .iter()
                    .filter(|n| n.id.starts_with("client-"))
                    .map(|n| n.label.as_str())
                    .collect::<Vec<_>>(),
                ["TUI 1", "TUI 2", "DESKTOP"]
            );
        }
        assert_eq!(a.view, DiagramView::Flat);
        assert_eq!(b.view, DiagramView::Isometric);
        assert_eq!(
            a.nodes.iter().map(|n| &n.id).collect::<Vec<_>>(),
            b.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn merge_retains_clients_and_changes_only_server_geometry() {
        for slide in build_deck().unwrap().slides {
            let p = slide.plan;
            p.validate().unwrap();
            assert_eq!(p.presentation_steps.len(), 4);
            let d: DiagramPlan = serde_json::from_value(p.actors[0].data.clone()).unwrap();
            assert_eq!(d.nodes.len(), 6);
            assert_eq!(d.links.len(), 3);
            for c in p
                .continuous_channels
                .iter()
                .filter(|c| c.property.starts_with("node.client-"))
            {
                assert!(
                    c.events.iter().all(|e| e.at_nanos() < 3 * BEAT),
                    "retained client must not restart during merge: {}",
                    c.property
                );
            }
        }
    }
}

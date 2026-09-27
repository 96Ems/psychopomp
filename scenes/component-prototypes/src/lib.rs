//! PROTOTYPE: three reusable visual pieces, four native compositions. Review
//! their appearance before promoting their provisional payloads into an API.
use anyhow::Result;
mod slideshow;
use kinograph::{
    author::{ActorHandle, PlanBuilder},
    component_prototype::*,
    plan::{DeckPlan, ScenePlan, SlidePlan},
};
pub use slideshow::build_slideshow_deck;

const SECOND: u64 = 1_000_000_000;
const BEAT: u64 = 3 * SECOND;
const INK: [u8; 3] = [235, 233, 227];
const MUTED: [u8; 3] = [127, 133, 141];
const ACCENT: [u8; 3] = [216, 168, 120];

fn words(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}
fn part(id: &str, text: &str, color: [u8; 3]) -> TextPart {
    TextPart {
        id: id.into(),
        text: text.into(),
        color,
        spans: vec![],
    }
}

pub fn build_deck() -> Result<DeckPlan> {
    let deck = DeckPlan {
        version: 1,
        id: "component-prototypes".into(),
        slides: vec![
            SlidePlan {
                title: "Typeset · make room".into(),
                plan: typesetting()?,
            },
            SlidePlan {
                title: "Collection · retain identity".into(),
                plan: collection()?,
            },
            SlidePlan {
                title: "Connectors · follow the endpoints".into(),
                plan: connectors()?,
            },
            SlidePlan {
                title: "Composition · the same pieces together".into(),
                plan: composition()?,
            },
        ],
    };
    deck.validate()?;
    Ok(deck)
}

fn scene(id: &str, steps: &[&str]) -> PlanBuilder {
    let mut p = PlanBuilder::new(id, steps.len() as u64 * BEAT);
    for (i, title) in steps.iter().enumerate() {
        let at = i as u64 * BEAT;
        p.presentation_step(
            format!("step-{i}"),
            *title,
            at,
            if i == 0 { 0 } else { at + 2 * SECOND },
        );
        p.cue(format!("step-{i}"), at, at + BEAT);
    }
    p
}
fn text(
    p: &mut PlanBuilder,
    id: &str,
    value: &str,
    origin: [f32; 2],
    size: f32,
    color: [u8; 3],
) -> Result<ActorHandle> {
    Ok(p.actor(
        id,
        TYPESET,
        TypesetPlan {
            origin,
            font: Font::Sans,
            font_size: size,
            parts: vec![part("text", value, color)],
            visible: words(&["text"]),
            events: vec![],
        },
    )?)
}
fn track(p: &mut PlanBuilder, actor: &ActorHandle, property: &str, values: &[f32]) {
    p.step_track(
        actor,
        property,
        values,
        kinograph::plan::SpringPlan::visual(0.4, 0.),
    );
}
fn footer(p: &mut PlanBuilder, index: usize, title: &str) -> Result<()> {
    text(
        p,
        "trial-label",
        &format!("0{index}   /   {title}"),
        [200., 878.],
        20.,
        MUTED,
    )?;
    Ok(())
}

fn typesetting() -> Result<ScenePlan> {
    let mut p = scene(
        "typeset-prototype",
        &[
            "Make room.",
            "Insert more; retain the surrounding words.",
            "Insert even; retain more.",
            "Remove only even.",
            "Return to the original sentence.",
        ],
    );
    p.actor(
        "sentence",
        TYPESET,
        TypesetPlan {
            origin: [200., 375.],
            font: Font::Sans,
            font_size: 112.,
            parts: vec![
                part("make", "Make ", INK),
                part("even", "even ", ACCENT),
                part("more", "more ", ACCENT),
                part("room", "room.", INK),
            ],
            visible: words(&["make", "room"]),
            events: [
                vec!["make", "more", "room"],
                vec!["make", "even", "more", "room"],
                vec!["make", "more", "room"],
                vec!["make", "room"],
            ]
            .into_iter()
            .enumerate()
            .map(|(i, ids)| TextEvent {
                at_nanos: (i + 1) as u64 * BEAT,
                visible: words(&ids),
            })
            .collect(),
        },
    )?;
    text(
        &mut p,
        "description",
        "The words you keep should feel like the same words.",
        [205., 565.],
        30.,
        MUTED,
    )?;
    footer(&mut p, 1, "Typesetting · measured inline reveals")?;
    Ok(p.finish()?)
}

fn snapshot(order: &[&str], arrangement: Arrangement, focus: Option<&str>) -> CollectionSnapshot {
    CollectionSnapshot {
        order: words(order),
        arrangement,
        focus: focus.map(str::to_owned),
    }
}
fn collection() -> Result<ScenePlan> {
    let mut p = scene(
        "collection-prototype",
        &[
            "Two retained items.",
            "Insert validate between them.",
            "Fold into a column and reorder.",
            "Return to the row.",
            "Remove only validate.",
        ],
    );
    text(
        &mut p,
        "title",
        "A place for every piece.",
        [200., 170.],
        66.,
        INK,
    )?;
    p.actor(
        "pipeline",
        COLLECTION,
        CollectionPlan {
            origin: [235., 385.],
            font: Font::Sans,
            font_size: 68.,
            gap: 80.,
            items: vec![
                part("parse", "parse", INK),
                part("validate", "validate", INK),
                part("persist", "persist", INK),
            ],
            initial: snapshot(&["parse", "persist"], Arrangement::Row, None),
            events: vec![
                CollectionEvent {
                    at_nanos: BEAT,
                    snapshot: snapshot(
                        &["parse", "validate", "persist"],
                        Arrangement::Row,
                        Some("validate"),
                    ),
                },
                CollectionEvent {
                    at_nanos: 2 * BEAT,
                    snapshot: snapshot(
                        &["persist", "parse", "validate"],
                        Arrangement::Column,
                        Some("validate"),
                    ),
                },
                CollectionEvent {
                    at_nanos: 3 * BEAT,
                    snapshot: snapshot(
                        &["parse", "validate", "persist"],
                        Arrangement::Row,
                        Some("parse"),
                    ),
                },
                CollectionEvent {
                    at_nanos: 4 * BEAT,
                    snapshot: snapshot(&["parse", "persist"], Arrangement::Row, None),
                },
            ],
        },
    )?;
    footer(&mut p, 2, "Collection · insert / arrange / remove")?;
    Ok(p.finish()?)
}

fn connect(
    p: &mut PlanBuilder,
    id: &str,
    from: (&str, &str, Side),
    to: (&str, &str, Side),
) -> Result<ActorHandle> {
    Ok(p.actor(
        id,
        CONNECTOR,
        ConnectorPlan {
            from: Anchor {
                actor: from.0.into(),
                item: from.1.into(),
                side: from.2,
            },
            to: Anchor {
                actor: to.0.into(),
                item: to.1.into(),
                side: to.2,
            },
            color: [103, 110, 120],
            accent: ACCENT,
        },
    )?)
}
fn connectors() -> Result<ScenePlan> {
    let mut p = scene(
        "connector-prototype",
        &[
            "Two independent collections.",
            "Connect their stable items.",
            "Move the destination collection; keep the connections.",
            "Trace one connection.",
            "Return the collection.",
        ],
    );
    text(&mut p, "title", "Stay connected.", [200., 150.], 76., INK)?;
    let labels = [
        ("request", "Request"),
        ("cache", "Cache"),
        ("database", "Database"),
    ];
    p.actor(
        "sources",
        COLLECTION,
        CollectionPlan {
            origin: [230., 370.],
            font: Font::Sans,
            font_size: 44.,
            gap: 84.,
            items: labels.map(|(id, label)| part(id, label, INK)).to_vec(),
            initial: snapshot(&["request", "cache", "database"], Arrangement::Column, None),
            events: vec![],
        },
    )?;
    let destinations = p.actor(
        "destinations",
        COLLECTION,
        CollectionPlan {
            origin: [1250., 370.],
            font: Font::Sans,
            font_size: 44.,
            gap: 64.,
            items: [("api", "API"), ("queue", "Queue"), ("store", "Store")]
                .map(|(id, label)| part(id, label, INK))
                .to_vec(),
            initial: snapshot(&["api", "queue", "store"], Arrangement::Column, None),
            events: vec![],
        },
    )?;
    track(
        &mut p,
        &destinations,
        "x",
        &[1250., 1250., 1130., 1130., 1250.],
    );
    track(&mut p, &destinations, "y", &[370., 370., 440., 440., 370.]);
    for (i, (from, to)) in [
        ("request", "api"),
        ("cache", "queue"),
        ("database", "store"),
    ]
    .into_iter()
    .enumerate()
    {
        let link = connect(
            &mut p,
            &format!("link-{i}"),
            ("sources", from, Side::Right),
            ("destinations", to, Side::Left),
        )?;
        track(&mut p, &link, "draw", &[0., 1., 1., 1., 1.]);
        if i == 1 {
            track(&mut p, &link, "signal", &[0., 0., 0., 1., 0.]);
            track(&mut p, &link, "signal-opacity", &[0., 0., 0., 1., 1.]);
        }
    }
    footer(&mut p, 3, "Connectors · sampled anchors / continuous paths")?;
    Ok(p.finish()?)
}

fn composition() -> Result<ScenePlan> {
    let mut p = scene(
        "composition-prototype",
        &[
            "Three pieces, one composition.",
            "Expand the sentence and the collection.",
            "Rearrange the collection; follow its item.",
            "Return without resetting anything.",
        ],
    );
    p.actor(
        "headline",
        TYPESET,
        TypesetPlan {
            origin: [200., 185.],
            font: Font::Sans,
            font_size: 76.,
            parts: vec![
                part("keep", "Keep ", INK),
                part("all", "all ", ACCENT),
                part("pieces", "the pieces.", INK),
            ],
            visible: words(&["keep", "pieces"]),
            events: vec![
                TextEvent {
                    at_nanos: BEAT,
                    visible: words(&["keep", "all", "pieces"]),
                },
                TextEvent {
                    at_nanos: 3 * BEAT,
                    visible: words(&["keep", "pieces"]),
                },
            ],
        },
    )?;
    p.actor(
        "parts",
        COLLECTION,
        CollectionPlan {
            origin: [235., 445.],
            font: Font::Sans,
            font_size: 44.,
            gap: 56.,
            items: [
                ("text", "Text"),
                ("layout", "Collection"),
                ("links", "Connector"),
            ]
            .map(|(id, label)| part(id, label, INK))
            .to_vec(),
            initial: snapshot(&["text", "links"], Arrangement::Column, None),
            events: vec![
                CollectionEvent {
                    at_nanos: BEAT,
                    snapshot: snapshot(
                        &["text", "layout", "links"],
                        Arrangement::Column,
                        Some("layout"),
                    ),
                },
                CollectionEvent {
                    at_nanos: 2 * BEAT,
                    snapshot: snapshot(
                        &["links", "text", "layout"],
                        Arrangement::Row,
                        Some("layout"),
                    ),
                },
                CollectionEvent {
                    at_nanos: 3 * BEAT,
                    snapshot: snapshot(&["text", "links"], Arrangement::Column, None),
                },
            ],
        },
    )?;
    p.actor(
        "outcome",
        TYPESET,
        TypesetPlan {
            origin: [1250., 575.],
            font: Font::Sans,
            font_size: 62.,
            parts: vec![part("together", "Together.", INK)],
            visible: words(&["together"]),
            events: vec![],
        },
    )?;
    let link = connect(
        &mut p,
        "composition-link",
        ("parts", "links", Side::Bottom),
        ("outcome", "together", Side::Left),
    )?;
    track(&mut p, &link, "signal", &[0., 1., 0., 1.]);
    p.continuous(&link, "signal-opacity", 1.);
    footer(&mut p, 4, "Composition · the same components, reused")?;
    Ok(p.finish()?)
}

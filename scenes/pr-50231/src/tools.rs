//! Three lanes from what Effect's JSON Schema now emits to what reads it. In
//! the broken story each shape arrives and is rejected; after the fix the same
//! packets carry a named repair and land clean, and the empty object is checked
//! against every provider's request body.
use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, seconds},
    narration::Narration,
    plan::ScenePlan,
    stage::{StageActor, StageElement, StagePlan, StagePost},
    tone::Tone,
};

use crate::{FAILURE, MARK, SEND, TICK, chip, footer, header, sound, span};

const ROWS: [f32; 3] = [340.0, 500.0, 660.0];
const SOURCE_X: f32 = 560.0;
const RESULT_X: f32 = 1450.0;
const FIX_X: f32 = 1065.0;
const PROVIDERS: [(&str, [f32; 2]); 4] = [
    ("OpenAI Chat", [1300.0, 785.0]),
    ("OpenAI Responses", [1600.0, 785.0]),
    ("Anthropic", [1300.0, 865.0]),
    ("Gemini", [1600.0, 865.0]),
];

/// One lane: what Effect emits and from what, who reads it, how it fails, and
/// what the named repair delivers.
struct Lane {
    shape: &'static str,
    origin: &'static str,
    consumer: &'static str,
    broken: (&'static str, Tone),
    fixed: &'static str,
    repair: &'static str,
}

const LANES: [Lane; 3] = [
    Lane {
        shape: "\"$ref\": \"#/$defs/Lookup%20Input\"",
        origin: "definition key: \"Lookup Input\"",
        consumer: "Code Mode",
        broken: ("$ref not found", Tone::Error),
        fixed: "→ \"Lookup Input\"",
        repair: "parseUriFragment",
    },
    Lane {
        shape: "number | \"NaN\" | \"Infinity\" | \"-Infinity\"",
        origin: "Schema.Number",
        consumer: "Code Mode",
        broken: ("number | string", Tone::Warning),
        fixed: "number",
        repair: "referencePolicy",
    },
    Lane {
        shape: "{ \"not\": { \"type\": \"null\" } }",
        origin: "Schema.Struct({})",
        consumer: "providers",
        broken: ("not an object", Tone::Error),
        fixed: "{ \"type\": \"object\", \"properties\": {} }",
        repair: "toEncoded + normalize",
    },
];

fn stage_plan() -> StagePlan {
    let mut elements = vec![
        StageElement::label(
            "col-source",
            [SOURCE_X, 262.0, 0.0],
            20.0,
            &[("effect rc.117 emits", Tone::Muted)],
        ),
        StageElement::label(
            "col-result",
            [RESULT_X, 262.0, 0.0],
            20.0,
            &[("read by", Tone::Muted)],
        ),
    ];
    for (index, lane) in LANES.iter().enumerate() {
        let y = ROWS[index];
        elements.extend([
            StageElement::card(
                &format!("source-{index}"),
                [SOURCE_X, y, 0.0],
                [760.0, 100.0],
                lane.shape,
            )
            .statuses(&[(lane.origin, Tone::Muted)]),
            StageElement::card(
                &format!("result-{index}"),
                [RESULT_X, y, 0.0],
                [520.0, 100.0],
                lane.consumer,
            )
            .statuses(&[
                ("·", Tone::Muted),
                (lane.broken.0, lane.broken.1),
                (lane.fixed, Tone::Success),
            ]),
            StageElement::beam(
                &format!("lane-{index}"),
                &format!("source-{index}"),
                &format!("result-{index}"),
            ),
            StageElement::packet(&format!("broken-{index}"), &format!("lane-{index}"))
                .tone(Tone::Request),
            StageElement::packet(&format!("fixed-{index}"), &format!("lane-{index}"))
                .tone(Tone::Success),
            StageElement::label(
                &format!("repair-{index}"),
                [FIX_X, y - 32.0, -2.0],
                17.0,
                &[(lane.repair, Tone::Accent)],
            ),
        ]);
    }
    elements.push(StageElement::label(
        "literals",
        [RESULT_X, 580.0, 0.0],
        17.0,
        &[
            (
                "Schema.Literals([\"NaN\", \"Infinity\", \"-Infinity\"])",
                Tone::Plain,
            ),
            (" → kept", Tone::Success),
        ],
    ));
    for (index, (name, [x, y])) in PROVIDERS.iter().enumerate() {
        elements.push(StageElement::card(
            &format!("provider-{index}"),
            [*x, *y, 0.0],
            [280.0, 64.0],
            name,
        ));
        elements.push(
            StageElement::beam(
                &format!("receive-{index}"),
                "result-2",
                &format!("provider-{index}"),
            )
            .tone(Tone::Success),
        );
    }
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

pub fn build(narration: &Narration) -> Result<ScenePlan> {
    let reading = narration.reading(
        seconds(0.5),
        [
            ("tools-before", seconds(1.0)),
            ("tools-after", seconds(0.8)),
        ],
    )?;
    let mut scene = PlanBuilder::new("tools", reading.duration());
    let [before, after] = reading.place(&mut scene);
    let b = |phrase: &str| before.at(phrase);
    let a = |phrase: &str| after.at(phrase);
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;

    header(sc, "2 · tool schemas", Some(seconds(0.15)))?;
    let mut before_chip = chip(sc, "chip-before", Tone::Muted, "before")?;
    before_chip.show(sc, seconds(0.4));
    s.channel(sc, "camera.z", -60.0);
    s.to(sc, "camera.z", 0, 0.0, 1.6);

    // Establish the lanes, then let them rest.
    let tools = b("tool schemas");
    for column in ["col-source", "col-result"] {
        s.fade_in(sc, column, tools, 1.0, 0.4);
    }
    let writes = b("now writes");
    for index in 0..LANES.len() {
        let stagger = seconds(0.12 * index as f64);
        s.settle_in(sc, &format!("source-{index}"), tools + stagger);
        s.settle_in(
            sc,
            &format!("result-{index}"),
            writes.saturating_sub(seconds(0.5)) + stagger,
        );
        let link = format!("lane-{index}");
        let contact = s.connect(sc, &link, writes + seconds(0.15 * index as f64), 0.45);
        s.to(
            sc,
            &format!("{link}.flow"),
            contact + seconds(0.7),
            0.0,
            0.45,
        );
        sc.media(sound(&format!("lane-{index}"), TICK, contact, -26.0));
    }

    // Each shape arrives and its reader rejects it.
    for (index, (at, red)) in [
        (b("reference names"), true),
        (b("plain numbers") + seconds(0.3), false),
        (b("stops being").saturating_sub(seconds(0.94)), true),
    ]
    .into_iter()
    .enumerate()
    {
        let result = format!("result-{index}");
        let arrival = s.send(sc, &format!("broken-{index}"), at, 0.6);
        s.to(sc, &format!("{result}.status"), arrival, 1.0, 0.3);
        sc.media(sound(&format!("broken-send-{index}"), SEND, at, -20.0));
        if red {
            s.hit(sc, &format!("{result}.alarm"), arrival, 0.9, 0.45);
            sc.media(sound(
                &format!("broken-land-{index}"),
                FAILURE,
                arrival,
                -16.0,
            ));
        } else {
            s.hit(sc, &format!("{result}.flash"), arrival, 0.4, 0.0);
            sc.media(sound(&format!("broken-land-{index}"), TICK, arrival, -20.0));
        }
    }
    let mut footer_before = footer(
        sc,
        "footer-before",
        vec![
            span("same schemas, ", Tone::Plain),
            span("different JSON", Tone::Error),
        ],
    )?;
    footer_before.type_in(sc, b("object at all"), 45.0, 0.5);

    // Reset the readers; the same packets replay with their repairs.
    let switch = before.end() + seconds(0.15);
    before_chip.hide(sc, switch);
    footer_before.hide(sc, switch);
    let mut after_chip = chip(sc, "chip-after", Tone::Success, "after the fix")?;
    after_chip.show(sc, switch + seconds(0.25));
    for index in 0..LANES.len() {
        let result = format!("result-{index}");
        s.to(sc, &format!("{result}.alarm"), switch, 0.0, 0.4);
        s.to(
            sc,
            &format!("{result}.status"),
            switch + seconds(0.1 * index as f64),
            0.0,
            0.35,
        );
    }
    for (index, (repair, land)) in [
        (a("decode the names"), a("decode") + seconds(0.6)),
        (a("show effects"), a("just a number")),
        (a("empty tools"), a("real empty")),
    ]
    .into_iter()
    .enumerate()
    {
        s.type_in(sc, &format!("repair-{index}"), repair, 50.0);
        let result = format!("result-{index}");
        let arrival = s.send_arriving(sc, &format!("fixed-{index}"), land, 0.55);
        s.to(sc, &format!("{result}.status"), arrival, 2.0, 0.3);
        s.hit(sc, &format!("{result}.flash"), arrival, 0.55, 0.0);
        s.hit(sc, &format!("{result}.glow"), arrival, 0.5, 0.0);
        sc.media(sound(
            &format!("fixed-send-{index}"),
            SEND,
            land.saturating_sub(seconds(0.55)),
            -20.0,
        ));
        sc.media(sound(&format!("fixed-land-{index}"), MARK, arrival, -20.0));
    }
    s.type_in(sc, "literals", a("keep strings"), 70.0);

    // Checked against each provider's request body.
    let checked = a("checked against");
    for index in 0..PROVIDERS.len() {
        let at = checked + seconds(0.12 * index as f64);
        s.settle_in(sc, &format!("provider-{index}"), at);
        let contact = s.connect(sc, &format!("receive-{index}"), at + seconds(0.15), 0.35);
        s.to(
            sc,
            &format!("receive-{index}.flow"),
            contact + seconds(0.6),
            0.0,
            0.45,
        );
        sc.media(sound(&format!("receive-{index}"), TICK, contact, -24.0));
    }
    let mut footer_after = footer(
        sc,
        "footer-after",
        vec![
            span("same schemas, ", Tone::Plain),
            span("the shapes each provider expects", Tone::Success),
        ],
    )?;
    footer_after.type_in(sc, checked + seconds(0.8), 50.0, 0.6);
    scene.finish().context("tools")
}

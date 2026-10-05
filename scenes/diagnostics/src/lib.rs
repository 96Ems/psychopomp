//! IDE annotation showroom: an Effect program that cannot run yet. A caret
//! rests after `Effect.runPromise`, an error wave draws under it, an Inlay Hint
//! reveals the program's inferred type, a Hover Card explains the missing
//! requirement while the caret selects where it comes from, and the fix lands
//! as a Stepped Diff: `Effect.provide` opens a row, the error moves down with
//! its line, and clears.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, seconds},
    callout::{CalloutActor, CalloutAnchorPlan, CalloutPlan, CalloutSide},
    caption::CaptionSpanPlan,
    editor::diff::{Diff, add, keep},
    ide::{CursorActor, CursorPlan, DiagnosticActor, DiagnosticPlan, HoverActor, HoverPlan},
    plan::ScenePlan,
    tone::Tone,
};

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

/// Line indices in the diff below.
const PROGRAM: usize = 3;
const SERVICE: usize = 4;
const PROVIDE: usize = 9;
const RUN: usize = 10;

pub fn build() -> Result<ScenePlan> {
    let duration = seconds(13.0);
    let mut scene = PlanBuilder::new("diagnostics", duration);
    let at = seconds;

    let diff = Diff {
        file_name: "program.ts",
        lines: vec![
            keep("import { Effect } from \"effect\""),
            keep("import { Database, DatabaseLive } from \"./db\""),
            keep(""),
            keep("const program = Effect.gen(function* () {")
                .range("name", "program")
                .inlay("type", "name", ": Effect<User, NotFound, Database>"),
            keep("  const db = yield* Database").range("service", "Database"),
            keep("  return yield* db.find(\"42\")"),
            keep("})"),
            keep(""),
            keep("program.pipe("),
            add(1, "  Effect.provide(DatabaseLive),")
                .range("provide", "Effect.provide(DatabaseLive),"),
            keep("  Effect.runPromise,").range("run", "Effect.runPromise"),
            keep(")"),
        ],
    };
    let fix = at(7.8);
    let editor = diff.declare(&mut scene, &[fix], 0, true)?;
    editor.target(&mut scene, "run", RUN, "run")?;
    editor.target(&mut scene, "service", SERVICE, "service")?;
    editor.target(&mut scene, "provide", PROVIDE, "provide")?;
    editor.target(&mut scene, "name", PROGRAM, "name")?;

    // The caret rests where `Effect.runPromise` was just typed.
    let caret = CursorActor::declare(
        &mut scene,
        "caret",
        &CursorPlan::new("run", "run")
            .anchor("service", "service")
            .anchor("provide", "provide"),
    )?;
    caret.show(&mut scene, at(0.7));

    // The compiler objects: an error wave draws on under the call.
    let error = DiagnosticActor::declare(&mut scene, "error", &DiagnosticPlan::error("run"))?;
    error.show(&mut scene, at(1.5));

    // The inferred type appears inline: the program still needs a Database.
    let inferred = editor.inlay(&mut scene, "type");
    inferred.show(&mut scene, at(2.4));

    // Hovering the error explains it.
    let hover = HoverActor::declare(
        &mut scene,
        "why",
        &HoverPlan::new("run")
            .code("const program: Effect<User, NotFound, Database>")
            .text(vec![
                span("Type ", Tone::Plain),
                span("'Database'", Tone::Error),
                span(" is not assignable to type ", Tone::Plain),
                span("'never'", Tone::Plain),
                span(".", Tone::Plain),
            ])
            .text(vec![
                span(
                    "runPromise needs every requirement provided.  ",
                    Tone::Muted,
                ),
                span("ts(2345)", Tone::Muted),
            ])
            .below(),
    )?;
    hover.show(&mut scene, at(3.9));

    // The caret selects where the requirement comes from.
    let selected = caret.select(&mut scene, "service", at(5.4), 0.45)?;
    hover.hide(&mut scene, at(6.7));
    caret.collapse(&mut scene, selected + seconds(1.0), 1.0);

    // The fix: provide the layer. The new row opens, the error rides down
    // with its line, and the caret lands at the end of the new call.
    caret.move_to(&mut scene, "provide", 1.0, fix + seconds(0.55))?;
    error.clear(&mut scene, fix + seconds(1.35));
    let mut provided = CalloutActor::declare(
        &mut scene,
        "provided",
        &CalloutPlan::new(
            CalloutAnchorPlan::Editor {
                id: "provide".into(),
                target: "provide".into(),
                edge: CalloutSide::Right,
                side: None,
            },
            vec![
                span("Database", Tone::Success),
                span(" provided", Tone::Plain),
            ],
        )
        .side(CalloutSide::Right)
        .reach(80.0)
        .tone(Tone::Success)
        .chip(),
    )?;
    caret.hide(&mut scene, fix + seconds(1.5));
    provided.show(&mut scene, fix + seconds(1.6));

    let steps = [
        ("error", "An error wave under the call", 0.0, 2.3),
        ("inlay", "The inferred type, inline", 2.4, 3.8),
        ("hover", "A hover card explains", 3.9, 5.2),
        ("select", "The requirement's source", 5.4, 7.6),
        ("fix", "Provide the layer", 7.8, 12.8),
    ];
    for (id, title, start, hold) in steps {
        scene.presentation_step(id, title, at(start), at(hold));
    }
    scene.cue("showroom", 0, duration);
    Ok(scene.finish()?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_showroom_builds() {
        let plan = super::build().unwrap();
        assert!(plan.actors.iter().any(|actor| actor.recipe == "diagnostic"));
        assert!(plan.actors.iter().any(|actor| actor.recipe == "hover-card"));
        assert!(plan.actors.iter().any(|actor| actor.recipe == "cursor"));
    }
}

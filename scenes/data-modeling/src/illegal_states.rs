use super::stage::*;
use anyhow::Result;
use kinograph::{
    code::{StyledSpan, SyntaxStyle},
    editor::{
        EditorInlineRevealPlan, EditorLinePlan, EditorPartPlan, EditorRecipePlan,
        EditorSemanticRangePlan, EditorSnapshotPlan,
    },
    plan::ScenePlan,
};

pub fn build() -> Result<ScenePlan> {
    let mut s = Stage::new(
        "illegal-states",
        "Your return type permits two bugs",
        &[
            "A nullable pair permits four presence combinations.",
            "Neither value, or both values: two cases we did not want.",
            "Use an explicit alternative: a User OR an Error.",
            "Two case shapes. The payloads can still have many values.",
        ],
    )?;
    let part = |id: &str, text: &str| EditorPartPlan {
        id: id.into(),
        spans: vec![StyledSpan::new(text, SyntaxStyle::Plain)],
    };
    let line = |id: &str, text: &str| EditorLinePlan {
        id: id.into(),
        parts: vec![part("body", text)],
        semantic_ranges: vec![],
    };
    let signature = EditorLinePlan {
        id: "signature".into(),
        parts: vec![
            part("prefix", "def getUser(id: Int): "),
            part("nullable-pair", "(User, Error)"),
            part("sum-type", "UserOrError"),
            part("suffix", " = ???"),
        ],
        semantic_ranges: ["nullable-pair", "sum-type"]
            .map(|id| EditorSemanticRangePlan {
                id: id.into(),
                first_part_id: id.into(),
                last_part_id: id.into(),
            })
            .to_vec(),
    };
    let final_ids = ["signature", "gap", "sum", "success", "failure"]
        .map(str::to_owned)
        .to_vec();
    let editor = s.scene.actor(
        "editor",
        "editor",
        EditorRecipePlan {
            file_name: "user-or-error.scala".into(),
            lines: vec![
                signature,
                line("gap", " "),
                line("sum", "sealed trait UserOrError"),
                line(
                    "success",
                    "case class ReceivedUser(user: User) extends UserOrError",
                ),
                line(
                    "failure",
                    "case class ReceivedError(error: Error) extends UserOrError",
                ),
            ],
            initial_line_ids: vec!["signature".into()],
            final_line_ids: final_ids.clone(),
            snapshots: vec![EditorSnapshotPlan {
                at_nanos: 2 * BEAT,
                line_ids: final_ids,
            }],
            line_height: 44.,
            entering_offset_x: 0.,
            focus_line_id: "signature".into(),
            focus_height: 44.,
            inline_reveal: EditorInlineRevealPlan {
                line_id: "signature".into(),
                range_id: "nullable-pair".into(),
                channel: Some("result-model".into()),
                reversed: true,
            },
            additional_inline_reveals: vec![EditorInlineRevealPlan {
                line_id: "signature".into(),
                range_id: "sum-type".into(),
                channel: Some("result-model".into()),
                reversed: false,
            }],
        },
    )?;
    s.track(&editor, "result-model", &[0., 0., 1., 1.]);
    s.scene.continuous(&editor, "panel-scale", 0.82);
    s.scene.continuous(&editor, "panel-y", -51.6);
    for (id, text, position) in [
        ("error-axis", "ERROR", [1055., 493.]),
        ("error-absent", "absent", [830., 534.]),
        ("error-present", "present", [1280., 534.]),
        ("user-axis", "USER", [425., 680.]),
        ("user-absent", "absent", [570., 605.]),
        ("user-present", "present", [570., 755.]),
    ] {
        let t = s.text(id, text, position, 22., MUTED)?;
        s.show(&t, 0, 2);
    }
    for (id, label, detail, x, y, valid) in [
        ("neither", "Neither", "null, null", 830., 605., false),
        ("failure", "Error", "failure", 1280., 605., true),
        ("success", "User", "success", 830., 755., true),
        ("both", "Both", "User, Error", 1280., 755., false),
    ] {
        let t = s.tile(id, label, detail, [x, y], [360., 108.])?;
        if valid {
            let destination = if id == "success" { 640. } else { 1280. };
            s.track(&t, "x", &[x, x, destination, destination]);
            s.track(&t, "y", &[y, y, 690., 690.]);
            s.track(&t, "emphasis", &[0., 0., 1., 1.]);
        } else {
            s.show(&t, 0, 2);
            let cross = s.text(&format!("invalid-{id}"), "×", [x + 205., y], 46., RED)?;
            s.show(&cross, 1, 2);
        }
    }
    for (id, text, x) in [
        ("success-case", "ReceivedUser(user)", 640.),
        ("failure-case", "ReceivedError(error)", 1280.),
    ] {
        let t = s.text(id, text, [x, 582.], 28., ORANGE)?;
        s.show(&t, 2, 4);
    }
    let or = s.text("or", "OR", [960., 690.], 32., ORANGE)?;
    s.show(&or, 2, 4);
    s.finish()
}

//! Sequence diagrams: participants with lifelines and time-ordered rows of
//! messages, notes and terminations. Rows are recipe-local identities revealed by
//! ordinary Continuous Channels; the recipe does not simulate the protocol it shows.
use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    tone::Tone,
};

pub const SEQUENCE_RECIPE: &str = "sequence";

/// Participant header height without and with a detail line.
pub const HEADER_HEIGHT: f32 = 58.0;
pub const HEADER_HEIGHT_WITH_DETAIL: f32 = 76.0;
/// Space between the header row and the first slot.
pub const LIFELINE_LEAD: f32 = 22.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SequencePlan {
    /// Top-left corner of the header row, in canvas pixels. Row asides are drawn
    /// right-aligned to the left of the first lifeline, clear of note boxes.
    pub origin: [f32; 2],
    /// Horizontal extent; participants are centered in equal columns across it.
    pub width: f32,
    /// Vertical distance between row slots.
    pub row_height: f32,
    /// Slots the lifelines extend through; defaults to the highest used slot + 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slots: Option<u32>,
    pub participants: Vec<SequenceParticipantPlan>,
    #[serde(default)]
    pub rows: Vec<SequenceRowPlan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SequenceParticipantPlan {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// One row. Rows with the same `slot` share a line, so a scene can replay a
/// "before" and an "after" in the same place by fading one set out.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SequenceRowPlan {
    /// An arrow between two lifelines, or a loop when `from` equals `to`.
    #[serde(rename_all = "camelCase")]
    Message {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<u32>,
        from: String,
        to: String,
        label: String,
        #[serde(default)]
        tone: Tone,
        /// Dashed, as for a response.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        reply: bool,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        aside: String,
    },
    /// A box spanning the lifelines of `over`.
    #[serde(rename_all = "camelCase")]
    Note {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<u32>,
        over: Vec<String>,
        text: String,
        #[serde(default)]
        tone: Tone,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        aside: String,
    },
    /// A participant stops: an X on its lifeline, which fades below this row.
    #[serde(rename_all = "camelCase")]
    End {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<u32>,
        participant: String,
        #[serde(default)]
        label: String,
        #[serde(default = "error_tone")]
        tone: Tone,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        aside: String,
    },
}

fn error_tone() -> Tone {
    Tone::Error
}

impl SequenceRowPlan {
    pub fn id(&self) -> &str {
        match self {
            Self::Message { id, .. } | Self::Note { id, .. } | Self::End { id, .. } => id,
        }
    }

    pub fn explicit_slot(&self) -> Option<u32> {
        match self {
            Self::Message { slot, .. } | Self::Note { slot, .. } | Self::End { slot, .. } => *slot,
        }
    }

    pub fn aside(&self) -> &str {
        match self {
            Self::Message { aside, .. } | Self::Note { aside, .. } | Self::End { aside, .. } => {
                aside
            }
        }
    }

    pub fn tone(&self) -> Tone {
        match self {
            Self::Message { tone, .. } | Self::Note { tone, .. } | Self::End { tone, .. } => *tone,
        }
    }
}

impl SequencePlan {
    /// The slot each row occupies: its explicit slot, or its index.
    pub fn row_slots(&self) -> Vec<u32> {
        self.rows
            .iter()
            .enumerate()
            .map(|(index, row)| row.explicit_slot().unwrap_or(index as u32))
            .collect()
    }

    pub fn slot_count(&self) -> u32 {
        let used = self
            .row_slots()
            .into_iter()
            .max()
            .map_or(0, |slot| slot + 1);
        self.slots.unwrap_or(used).max(used)
    }

    pub fn header_height(&self) -> f32 {
        if self.participants.iter().any(|p| !p.detail.is_empty()) {
            HEADER_HEIGHT_WITH_DETAIL
        } else {
            HEADER_HEIGHT
        }
    }

    /// Center x of each participant column, before the actor's `x` offset.
    pub fn participant_x(&self, index: usize) -> f32 {
        let column = self.width / self.participants.len() as f32;
        self.origin[0] + column * (index as f32 + 0.5)
    }

    /// Center y of a slot, before the actor's `y` offset.
    pub fn slot_y(&self, slot: u32) -> f32 {
        self.origin[1]
            + self.header_height()
            + LIFELINE_LEAD
            + self.row_height * (slot as f32 + 0.5)
    }

    /// Where the lifelines end, before the actor's `y` offset.
    pub fn lifeline_bottom(&self) -> f32 {
        self.slot_y(self.slot_count().max(1) - 1) + self.row_height * 0.5
    }

    pub fn participant_index(&self, id: &str) -> Option<usize> {
        self.participants.iter().position(|p| p.id == id)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "sequence origin must be finite"
        );
        ensure!(
            (240.0..=1920.0).contains(&self.width),
            "sequence width must be between 240 and 1920"
        );
        ensure!(
            (24.0..=240.0).contains(&self.row_height),
            "sequence row height must be between 24 and 240"
        );
        ensure!(
            (1..=8).contains(&self.participants.len()),
            "a sequence needs one to eight participants"
        );
        let mut participants = HashSet::new();
        for participant in &self.participants {
            identifier("participant", &participant.id)?;
            ensure!(
                participants.insert(participant.id.as_str()),
                "sequence participant '{}' is declared twice",
                participant.id
            );
            single_line("participant label", &participant.label, 48)?;
            ensure!(
                !participant.label.trim().is_empty(),
                "participant labels cannot be empty"
            );
            single_line("participant detail", &participant.detail, 48)?;
        }
        ensure!(self.rows.len() <= 64, "a sequence can have at most 64 rows");
        let known = |id: &str, what: &str, row: &str| -> Result<()> {
            ensure!(
                participants.contains(id),
                "sequence row '{row}' {what} unknown participant '{id}'"
            );
            Ok(())
        };
        let mut rows = HashSet::new();
        for row in &self.rows {
            identifier("row", row.id())?;
            ensure!(
                rows.insert(row.id()),
                "sequence row '{}' is declared twice",
                row.id()
            );
            single_line("row aside", row.aside(), 24)?;
            match row {
                SequenceRowPlan::Message {
                    id,
                    from,
                    to,
                    label,
                    ..
                } => {
                    known(from, "starts at", id)?;
                    known(to, "ends at", id)?;
                    single_line("message label", label, 64)?;
                }
                SequenceRowPlan::Note { id, over, text, .. } => {
                    ensure!(
                        !over.is_empty(),
                        "note '{id}' must span at least one participant"
                    );
                    for participant in over {
                        known(participant, "spans", id)?;
                    }
                    single_line("note text", text, 80)?;
                    ensure!(!text.trim().is_empty(), "note '{id}' needs text");
                }
                SequenceRowPlan::End {
                    id,
                    participant,
                    label,
                    ..
                } => {
                    known(participant, "ends", id)?;
                    single_line("end label", label, 40)?;
                }
            }
        }
        let used = self
            .row_slots()
            .into_iter()
            .max()
            .map_or(0, |slot| slot + 1);
        if let Some(slots) = self.slots {
            ensure!(
                slots >= used,
                "sequence declares {slots} slots but rows use {used}"
            );
        }
        ensure!(
            self.slot_count() <= 32,
            "a sequence can have at most 32 slots"
        );
        Ok(())
    }
}

fn identifier(what: &str, id: &str) -> Result<()> {
    if id.is_empty() || id.chars().any(|c| c.is_whitespace() || c == '.') {
        bail!("sequence {what} ID '{id}' must be non-empty without whitespace or dots");
    }
    Ok(())
}

fn single_line(what: &str, text: &str, max_chars: usize) -> Result<()> {
    ensure!(
        !text.contains('\n'),
        "sequence {what} must be a single line"
    );
    ensure!(
        text.chars().count() <= max_chars,
        "sequence {what} '{text}' exceeds {max_chars} characters"
    );
    Ok(())
}

/// Authoring handle that declares each sequence channel once, with the recipe's
/// default as its initial value, and writes eased reveals by row identity.
pub struct SequenceActor {
    actor: ActorHandle,
    channels: HashMap<String, ContinuousHandle>,
}

/// Default reveal motion: a critically damped spring that settles in 0.6 s.
pub const REVEAL_SECONDS: f32 = 0.6;

impl SequenceActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &SequencePlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, SEQUENCE_RECIPE, plan)?;
        Ok(Self {
            actor,
            channels: HashMap::new(),
        })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    /// The channel for `property`, declared on first use with `initial`.
    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        self.channels
            .entry(property.to_owned())
            .or_insert_with(|| scene.continuous(&self.actor, property, initial))
            .clone()
    }

    /// Ease a whole-diagram property (`opacity`, `x`, `y`, `lifelines`).
    pub fn animate(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
        at_nanos: u64,
        target: f32,
        seconds: f32,
    ) {
        let channel = self.channel(scene, property, initial);
        scene.spring(&channel, at_nanos, target, seconds, 0.0);
    }

    /// Draw a row in: arrows travel, notes and marks appear.
    pub fn reveal(&mut self, scene: &mut PlanBuilder, row: &str, at_nanos: u64) {
        let channel = self.channel(scene, &format!("row.{row}.reveal"), 0.0);
        scene.spring(&channel, at_nanos, 1.0, REVEAL_SECONDS, 0.0);
    }

    /// Fade a revealed row to `opacity` without undrawing it.
    pub fn fade(&mut self, scene: &mut PlanBuilder, row: &str, at_nanos: u64, opacity: f32) {
        let channel = self.channel(scene, &format!("row.{row}.opacity"), 1.0);
        scene.spring(&channel, at_nanos, opacity, 0.45, 0.0);
    }

    /// Strike a row through, as for a result that is dropped or ignored.
    pub fn strike(&mut self, scene: &mut PlanBuilder, row: &str, at_nanos: u64) {
        let channel = self.channel(scene, &format!("row.{row}.strike"), 0.0);
        scene.spring(&channel, at_nanos, 1.0, 0.5, 0.0);
    }

    pub fn participant(
        &mut self,
        scene: &mut PlanBuilder,
        participant: &str,
        property: &str,
        initial: f32,
        at_nanos: u64,
        target: f32,
    ) {
        let channel = self.channel(
            scene,
            &format!("participant.{participant}.{property}"),
            initial,
        );
        scene.spring(&channel, at_nanos, target, 0.5, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> SequencePlan {
        serde_json::from_value(serde_json::json!({
            "origin": [240, 180],
            "width": 1440,
            "rowHeight": 80,
            "participants": [
                { "id": "client", "label": "client" },
                { "id": "server", "label": "server", "detail": "pid 4127" }
            ],
            "rows": [
                { "kind": "message", "id": "probe", "from": "client", "to": "server",
                  "label": "GET /api/info", "tone": "request" },
                { "kind": "message", "id": "missing", "from": "server", "to": "client",
                  "label": "404", "tone": "error", "reply": true, "aside": "0s" },
                { "kind": "end", "id": "killed", "participant": "server", "label": "SIGTERM" },
                { "kind": "note", "id": "after", "slot": 1, "over": ["client", "server"],
                  "text": "no signal is sent", "tone": "success" }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn rows_default_to_their_index_and_may_share_slots() {
        let plan = plan();
        plan.validate().unwrap();
        assert_eq!(plan.row_slots(), vec![0, 1, 2, 1]);
        assert_eq!(plan.slot_count(), 3);
        assert_eq!(plan.header_height(), HEADER_HEIGHT_WITH_DETAIL);
        assert_eq!(plan.participant_x(0), 240.0 + 360.0);
        assert_eq!(plan.participant_x(1), 240.0 + 1080.0);
        let first = plan.slot_y(0);
        assert_eq!(plan.slot_y(1) - first, 80.0);
        assert_eq!(plan.lifeline_bottom(), plan.slot_y(2) + 40.0);
    }

    #[test]
    fn end_rows_default_to_the_error_tone_and_round_trip() {
        let plan = plan();
        assert_eq!(plan.rows[2].tone(), Tone::Error);
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(json["rows"][0]["kind"], "message");
        assert_eq!(json["rowHeight"], 80.0);
        assert!(
            json["rows"][0].get("reply").is_none(),
            "false flags are omitted"
        );
        let decoded: SequencePlan = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, plan);
    }

    #[test]
    fn invalid_references_and_identities_are_rejected() {
        let mut unknown = plan();
        let SequenceRowPlan::Message { to, .. } = &mut unknown.rows[0] else {
            unreachable!()
        };
        *to = "daemon".into();
        assert!(
            unknown
                .validate()
                .unwrap_err()
                .to_string()
                .contains("unknown participant")
        );

        let mut repeated = plan();
        repeated.participants[1].id = "client".into();
        assert!(repeated.validate().is_err());

        let mut dotted = plan();
        let SequenceRowPlan::Note { id, .. } = &mut dotted.rows[3] else {
            unreachable!()
        };
        *id = "a.b".into();
        assert!(
            dotted.validate().is_err(),
            "dots would collide with channel names"
        );

        let mut too_few = plan();
        too_few.slots = Some(2);
        assert!(too_few.validate().is_err());

        let mut empty_note = plan();
        let SequenceRowPlan::Note { over, .. } = &mut empty_note.rows[3] else {
            unreachable!()
        };
        over.clear();
        assert!(empty_note.validate().is_err());

        let strict = serde_json::from_value::<SequencePlan>(serde_json::json!({
            "origin": [0, 0], "width": 800, "rowHeight": 60,
            "participants": [{ "id": "a", "label": "a" }],
            "rows": [{ "kind": "message", "id": "m", "from": "a", "to": "a", "label": "x", "colour": "red" }]
        }));
        assert!(strict.is_err(), "unknown row fields are rejected");
    }

    #[test]
    fn actor_helper_declares_each_channel_once() {
        let mut scene = PlanBuilder::new("sequence-demo", 5_000_000_000);
        let mut sequence = SequenceActor::declare(&mut scene, "flow", &plan()).unwrap();
        sequence.reveal(&mut scene, "probe", 1_000_000_000);
        sequence.reveal(&mut scene, "missing", 2_000_000_000);
        sequence.fade(&mut scene, "probe", 3_000_000_000, 0.3);
        sequence.fade(&mut scene, "probe", 4_000_000_000, 1.0);
        sequence.strike(&mut scene, "missing", 3_500_000_000);
        let plan = scene.finish().unwrap();
        let properties = plan
            .continuous_channels
            .iter()
            .map(|channel| (channel.property.as_str(), channel.events.len()))
            .collect::<Vec<_>>();
        assert_eq!(
            properties,
            vec![
                ("row.probe.reveal", 1),
                ("row.missing.reveal", 1),
                ("row.probe.opacity", 2),
                ("row.missing.strike", 1),
            ]
        );
    }
}

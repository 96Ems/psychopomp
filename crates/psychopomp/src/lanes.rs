//! Lanes: a DAW-style track view showing that a Scene Plan is just channels
//! over time. A seconds ruler, named lanes with keyframe diamonds and optional
//! value sparklines, cue brackets above the ruler, and a scrubbing playhead.
//! `LanesPlan::from_scene_plan` builds one from a plan's own channels.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder},
    axis::AxisPlan,
    math::easing::Ease,
    plan::{ContinuousChannelPlan, ScalarPlan, ScenePlan, compile_channels},
    plot::{DRAW_EASE, valid_id},
    timeline::PropertyId,
    tone::Tone,
};

pub const LANES_RECIPE: &str = "lanes";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanesPlan {
    /// Top-left corner of the view (the top of the cue row), in canvas pixels.
    pub origin: [f32; 2],
    pub width: f32,
    /// The time axis, in seconds.
    pub time: AxisPlan,
    /// Width of the lane-name column left of the tracks.
    #[serde(default = "default_label_width")]
    pub label_width: f32,
    #[serde(default = "default_lane_height")]
    pub lane_height: f32,
    pub lanes: Vec<LanePlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cues: Vec<LaneCuePlan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanePlan {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Tone::is_default")]
    pub tone: Tone,
    /// Keyframe times in seconds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<f32>,
    /// `[seconds, value]` points in time order, drawn as a sparkline scaled to
    /// the lane's own value range.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curve: Vec<[f32; 2]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaneCuePlan {
    pub id: String,
    pub start: f32,
    pub end: f32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

fn default_label_width() -> f32 {
    400.0
}

fn default_lane_height() -> f32 {
    52.0
}

/// Height of the cue row and time ruler above the first lane.
pub const LANES_HEADER: f32 = 104.0;

impl LanePlan {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tone: Tone::Plain,
            keys: Vec::new(),
            curve: Vec::new(),
        }
    }

    pub fn keys(mut self, keys: impl IntoIterator<Item = f32>) -> Self {
        self.keys = keys.into_iter().collect();
        self
    }

    pub fn curve(mut self, curve: Vec<[f32; 2]>) -> Self {
        self.curve = curve;
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// The lowest and highest curve value, or `None` without a curve.
    pub fn value_range(&self) -> Option<[f32; 2]> {
        let mut values = self.curve.iter().map(|point| point[1]);
        let first = values.next()?;
        Some(values.fold([first, first], |[lo, hi], v| [lo.min(v), hi.max(v)]))
    }
}

impl LanesPlan {
    pub fn new(origin: [f32; 2], width: f32, time: AxisPlan) -> Self {
        Self {
            origin,
            width,
            time,
            label_width: default_label_width(),
            lane_height: default_lane_height(),
            lanes: Vec::new(),
            cues: Vec::new(),
        }
    }

    pub fn lane(mut self, lane: LanePlan) -> Self {
        self.lanes.push(lane);
        self
    }

    pub fn cue(
        mut self,
        id: impl Into<String>,
        [start, end]: [f32; 2],
        label: impl Into<String>,
    ) -> Self {
        self.cues.push(LaneCuePlan {
            id: id.into(),
            start,
            end,
            label: label.into(),
        });
        self
    }

    /// One lane per channel of `plan` that `label` names: keyframes at its
    /// event times and a sparkline sampled from the same compiled Property
    /// Track the renderer plays, plus the plan's cues. The time axis spans the
    /// whole plan with one tick per second.
    pub fn from_scene_plan(
        plan: &ScenePlan,
        origin: [f32; 2],
        width: f32,
        label: impl Fn(&ContinuousChannelPlan) -> Option<String>,
    ) -> Result<Self> {
        let duration = plan.duration_nanos as f32 / 1e9;
        let selected = plan
            .continuous_channels
            .iter()
            .filter_map(|channel| label(channel).map(|label| (channel, label)))
            .collect::<Vec<_>>();
        ensure!(!selected.is_empty(), "no channels were selected");
        let timeline = compile_channels(
            selected
                .iter()
                .map(|(channel, _)| (*channel, PropertyId::new(channel.id.clone()))),
            plan.duration_nanos,
            |value| match value {
                ScalarPlan::Literal(value) => Ok(*value),
                ScalarPlan::Target(target) => {
                    bail!("lanes cannot sample semantic target '{}'", target.target_id)
                }
            },
        )?;
        const SAMPLES: usize = 241;
        let mut lanes = Self::new(
            origin,
            width,
            AxisPlan::new([0.0, duration]).every(1.0).unit("s"),
        );
        for (channel, label) in selected {
            let property = PropertyId::new(channel.id.clone());
            let curve = (0..SAMPLES)
                .map(|index| {
                    let t = duration * index as f32 / (SAMPLES - 1) as f32;
                    let value = timeline
                        .sample_at(&property, f64::from(t))
                        .context("compiled channel")?
                        .position;
                    Ok([t, value])
                })
                .collect::<Result<Vec<_>>>()?;
            let mut keys = channel
                .events
                .iter()
                .map(|event| event.at_nanos() as f32 / 1e9)
                .collect::<Vec<_>>();
            keys.dedup();
            lanes = lanes.lane(LanePlan::new(&channel.id, label).keys(keys).curve(curve));
        }
        for cue in &plan.cues {
            lanes = lanes.cue(
                &cue.id,
                [cue.start_nanos as f32 / 1e9, cue.end_nanos as f32 / 1e9],
                &cue.id,
            );
        }
        Ok(lanes)
    }

    pub fn find_lane(&self, id: &str) -> Option<&LanePlan> {
        self.lanes.iter().find(|lane| lane.id == id)
    }

    /// Total height from the cue row to the bottom of the last lane.
    pub fn height(&self) -> f32 {
        LANES_HEADER + self.lane_height * self.lanes.len() as f32
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "lanes origin must be finite"
        );
        ensure!(
            (80.0..=600.0).contains(&self.label_width)
                && self.width.is_finite()
                && self.width >= self.label_width + 200.0,
            "lanes need a 80–600 px label column and at least 200 px of track"
        );
        ensure!(
            (24.0..=160.0).contains(&self.lane_height),
            "lane height must be between 24 and 160"
        );
        self.time.validate("time")?;
        ensure!(
            (1..=24).contains(&self.lanes.len()),
            "a lanes view has one to 24 lanes"
        );
        let mut ids = HashSet::new();
        for lane in &self.lanes {
            valid_id(&lane.id, "lane")?;
            ensure!(ids.insert(&lane.id), "duplicate lane id '{}'", lane.id);
            ensure!(
                lane.label.chars().count() <= 48 && !lane.label.contains('\n'),
                "lane '{}' label is one line of at most 48 characters",
                lane.id
            );
            ensure!(
                lane.keys.len() <= 512 && lane.keys.iter().all(|t| t.is_finite()),
                "lane '{}' has at most 512 finite keys",
                lane.id
            );
            ensure!(
                lane.curve.len() <= 2048
                    && lane.curve.iter().flatten().all(|v| v.is_finite())
                    && lane.curve.windows(2).all(|pair| pair[0][0] <= pair[1][0]),
                "lane '{}' curve has at most 2048 finite points in time order",
                lane.id
            );
        }
        let mut cues = HashSet::new();
        for cue in &self.cues {
            valid_id(&cue.id, "cue")?;
            ensure!(cues.insert(&cue.id), "duplicate cue id '{}'", cue.id);
            ensure!(
                cue.start.is_finite() && cue.end.is_finite() && cue.start < cue.end,
                "cue '{}' needs a finite start before its end",
                cue.id
            );
        }
        ensure!(self.cues.len() <= 16, "a lanes view has at most 16 cues");
        Ok(())
    }

    /// Whether `property` names a lanes channel, as preflight checks.
    pub fn accepts(&self, property: &str) -> bool {
        if matches!(
            property,
            "opacity" | "x" | "y" | "reveal" | "playhead" | "playhead.opacity"
        ) {
            return true;
        }
        property
            .strip_prefix("lane.")
            .and_then(|rest| {
                rest.strip_suffix(".opacity")
                    .or_else(|| rest.strip_suffix(".emphasis"))
            })
            .is_some_and(|id| self.find_lane(id).is_some())
    }
}

/// Authoring handle for one lanes actor. Channels are declared on first use.
pub struct LanesActor {
    actor: ActorHandle,
}

impl LanesActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &LanesPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, LANES_RECIPE, plan)?;
        Ok(Self { actor })
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// `lane.<lane>.<property>`: `opacity` or `emphasis`.
    pub fn lane_channel(
        &mut self,
        scene: &mut PlanBuilder,
        lane: &str,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        self.channel(scene, &format!("lane.{lane}.{property}"), initial)
    }

    /// Fade in and build the view (ruler, then lanes top to bottom, then cues)
    /// over `seconds`.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64, seconds: f32) {
        crate::caption::show(scene, &self.actor, at_nanos);
        let reveal = self.channel(scene, "reveal", 0.0);
        scene.ease(&reveal, at_nanos, 1.0, seconds, DRAW_EASE);
    }

    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        crate::caption::hide(scene, &self.actor, at_nanos);
    }

    /// Scrub the playhead from `from` to `to` seconds at constant speed,
    /// fading it in as it starts. Returns when it arrives.
    pub fn scrub(
        &mut self,
        scene: &mut PlanBuilder,
        [from, to]: [f32; 2],
        at_nanos: u64,
        seconds: f32,
    ) -> u64 {
        let playhead = self.channel(scene, "playhead", from);
        let opacity = self.channel(scene, "playhead.opacity", 0.0);
        scene.set(&playhead, at_nanos, from);
        scene.ease(&playhead, at_nanos, to, seconds, Ease::Linear);
        scene.spring(&opacity, at_nanos, 1.0, 0.3, 0.0);
        at_nanos + crate::author::whole_millis(seconds)
    }

    /// Spring a lane's emphasis (0 rest, 1 the lane being explained).
    pub fn emphasize(&mut self, scene: &mut PlanBuilder, lane: &str, at_nanos: u64, emphasis: f32) {
        let channel = self.lane_channel(scene, lane, "emphasis", 0.0);
        scene.spring(&channel, at_nanos, emphasis, 0.35, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::SECOND;

    fn plan() -> LanesPlan {
        LanesPlan::new(
            [160.0, 200.0],
            1600.0,
            AxisPlan::new([0.0, 4.0]).every(1.0).unit("s"),
        )
        .lane(LanePlan::new("card.x", "card.x").keys([0.5, 2.0]))
        .lane(LanePlan::new("card.opacity", "card.opacity").curve(vec![
            [0.0, 0.0],
            [1.0, 1.0],
            [4.0, 1.0],
        ]))
        .cue("intro", [0.0, 2.0], "intro")
    }

    #[test]
    fn lanes_round_trip_and_accept_only_known_channels() {
        let plan = plan();
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(serde_json::from_value::<LanesPlan>(json).unwrap(), plan);
        assert_eq!(plan.height(), LANES_HEADER + 2.0 * 52.0);
        assert_eq!(plan.lanes[1].value_range(), Some([0.0, 1.0]));
        // Lane ids may contain dots; the property is matched by prefix and suffix.
        for property in [
            "playhead",
            "reveal",
            "lane.card.x.emphasis",
            "lane.card.opacity.opacity",
        ] {
            assert!(plan.accepts(property), "{property}");
        }
        for property in [
            "lane.card.y.opacity",
            "lane.card.x.glow",
            "cue.intro.opacity",
        ] {
            assert!(!plan.accepts(property), "{property}");
        }
    }

    #[test]
    fn invalid_lanes_are_rejected() {
        let mut duplicate = plan();
        duplicate.lanes[1].id = "card.x".into();
        assert!(duplicate.validate().is_err());
        let mut backwards = plan();
        backwards.cues[0].end = -1.0;
        assert!(backwards.validate().is_err());
        let mut unordered = plan();
        unordered.lanes[1].curve.swap(0, 2);
        assert!(unordered.validate().is_err());
    }

    #[test]
    fn a_scene_plan_becomes_lanes_of_its_own_channels() {
        let mut scene = PlanBuilder::new("source", 3 * SECOND);
        let card = scene.actor("card", "text", serde_json::json!({})).unwrap();
        let x = scene.channel(&card, "x", 0.0);
        let opacity = scene.channel(&card, "opacity", 0.0);
        scene.spring(&x, SECOND / 2, 100.0, 0.4, 0.0);
        scene.ease(&opacity, SECOND, 1.0, 1.0, Ease::Linear);
        scene.cue("intro", 0, 2 * SECOND);
        let source = scene.finish().unwrap();
        let lanes = LanesPlan::from_scene_plan(&source, [100.0, 100.0], 1600.0, |channel| {
            (channel.property != "opacity").then(|| channel.property.clone())
        })
        .unwrap();
        lanes.validate().unwrap();
        assert_eq!(lanes.lanes.len(), 1);
        let lane = &lanes.lanes[0];
        assert_eq!((lane.id.as_str(), lane.label.as_str()), ("card.x", "x"));
        assert_eq!(lane.keys, vec![0.5]);
        // The sparkline is the compiled spring: at rest before, settled after.
        assert_eq!(lane.curve.first().unwrap()[1], 0.0);
        assert!((lane.curve.last().unwrap()[1] - 100.0).abs() < 0.2);
        assert_eq!(lanes.time.ticks, vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(lanes.cues[0].end, 2.0);
    }
}

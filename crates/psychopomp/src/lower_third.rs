//! Lower Thirds: a speaker's or subject's name and role beside an accent bar,
//! as broadcast captions introduce someone. The bar draws up, the name slides
//! out from behind it, and the role follows; leaving reverses the order. Every
//! phase is its own Continuous Channel, so a lower third presents natively.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, DRAW_SECONDS, PlanBuilder, STAGGER},
    math::easing::Ease,
    tone::Tone,
};

pub const LOWER_THIRD_RECIPE: &str = "lower-third";

/// `opacity`, `x`/`y` offsets, and the three phases, each 0 to 1: `bar`
/// (draws up from its foot), `name`, and `role` (each slides out from
/// behind the bar).
pub const LOWER_THIRD_PROPERTIES: [&str; 6] = ["opacity", "x", "y", "bar", "name", "role"];

const DEFAULT_SIZE: f32 = 46.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LowerThirdPlan {
    /// The bar's left edge and the name line's vertical center.
    pub origin: [f32; 2],
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The bar's color.
    #[serde(default = "accent", skip_serializing_if = "is_accent")]
    pub tone: Tone,
    /// The name's font size; the role is set at about half.
    #[serde(default = "default_size", skip_serializing_if = "is_default_size")]
    pub size: f32,
}

fn accent() -> Tone {
    Tone::Accent
}

fn is_accent(tone: &Tone) -> bool {
    *tone == Tone::Accent
}

fn default_size() -> f32 {
    DEFAULT_SIZE
}

fn is_default_size(size: &f32) -> bool {
    *size == DEFAULT_SIZE
}

impl LowerThirdPlan {
    pub fn new(origin: [f32; 2], name: impl Into<String>) -> Self {
        Self {
            origin,
            name: name.into(),
            role: None,
            tone: Tone::Accent,
            size: DEFAULT_SIZE,
        }
    }

    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// The role's font size.
    pub fn role_size(&self) -> f32 {
        (self.size * 0.46).round()
    }

    /// The bar's `[top, bottom]`, spanning the name and any role.
    pub fn bar_span(&self) -> [f32; 2] {
        let top = self.origin[1] - self.size * 0.62;
        let bottom = match self.role {
            Some(_) => self.role_center() + self.role_size() * 0.62,
            None => self.origin[1] + self.size * 0.62,
        };
        [top, bottom]
    }

    pub fn bar_width(&self) -> f32 {
        (self.size * 0.11).round().max(3.0)
    }

    /// Left edge of the name and role.
    pub fn text_left(&self) -> f32 {
        self.origin[0] + self.bar_width() + (self.size * 0.4).round()
    }

    pub fn role_center(&self) -> f32 {
        self.origin[1] + self.size * 0.62 + self.role_size() * 0.72
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "lower third origin must be finite"
        );
        ensure!(
            (16.0..=120.0).contains(&self.size),
            "lower third size must be between 16 and 120"
        );
        for (text, limit, what) in [
            (Some(&self.name), 60, "name"),
            (self.role.as_ref(), 90, "role"),
        ] {
            if let Some(text) = text {
                ensure!(
                    !text.trim().is_empty()
                        && !text.contains('\n')
                        && text.chars().count() <= limit,
                    "a lower third's {what} is one line of 1 to {limit} characters"
                );
            }
        }
        Ok(())
    }
}

/// Authoring handle for one lower third.
#[derive(Clone, Debug)]
pub struct LowerThirdActor {
    actor: ActorHandle,
}

impl LowerThirdActor {
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: &LowerThirdPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, LOWER_THIRD_RECIPE, plan)?;
        Ok(Self { actor })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    /// Draw the bar up, slide the name out from behind it, then the role.
    /// A lower third with a `show` starts hidden. Returns when it has landed.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> u64 {
        let bar = self.channel(scene, "bar", 0.0);
        scene.ease(&bar, at_nanos, 1.0, DRAW_SECONDS, Ease::DRAW);
        let name = self.channel(scene, "name", 0.0);
        scene.ease(&name, at_nanos + STAGGER, 1.0, 0.46, Ease::DRAW);
        let role = self.channel(scene, "role", 0.0);
        scene.ease(&role, at_nanos + 2 * STAGGER, 1.0, 0.44, Ease::DRAW);
        at_nanos + 800_000_000
    }

    /// Withdraw the role and name behind the bar, then retract it. Returns
    /// when it has gone.
    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> u64 {
        let role = self.channel(scene, "role", 1.0);
        scene.ease(&role, at_nanos, 0.0, 0.26, Ease::GLIDE);
        let name = self.channel(scene, "name", 1.0);
        scene.ease(&name, at_nanos + 60_000_000, 0.0, 0.28, Ease::GLIDE);
        let bar = self.channel(scene, "bar", 1.0);
        scene.ease(&bar, at_nanos + 300_000_000, 0.0, 0.26, Ease::GLIDE);
        at_nanos + 560_000_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::TrackEventPlan;

    #[test]
    fn lower_thirds_round_trip_and_reject_long_or_empty_text() {
        let plan = LowerThirdPlan::new([120.0, 880.0], "Dax Raad").role("opencode · co-founder");
        plan.validate().unwrap();
        let json = serde_json::to_value(&plan).unwrap();
        assert!(json.get("tone").is_none() && json.get("size").is_none());
        assert_eq!(
            serde_json::from_value::<LowerThirdPlan>(json).unwrap(),
            plan
        );
        assert!(LowerThirdPlan::new([0.0, 0.0], " ").validate().is_err());
        assert!(LowerThirdPlan::new([0.0, 0.0], "a\nb").validate().is_err());
        assert!(
            LowerThirdPlan::new([0.0, 0.0], "x".repeat(61))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn the_bar_spans_the_name_and_role_and_text_clears_it() {
        let plain = LowerThirdPlan::new([120.0, 880.0], "Kit");
        let with_role = plain.clone().role("explainers");
        assert!(with_role.bar_span()[1] > plain.bar_span()[1]);
        assert!(with_role.role_center() > with_role.origin[1]);
        assert!(plain.text_left() > plain.origin[0] + plain.bar_width());
    }

    #[test]
    fn showing_staggers_the_bar_name_and_role_and_hiding_reverses_them() {
        let mut scene = PlanBuilder::new("lower-third", 5_000_000_000);
        let mut third = LowerThirdActor::declare(
            &mut scene,
            "who",
            &LowerThirdPlan::new([120.0, 880.0], "Kit").role("explainers"),
        )
        .unwrap();
        assert_eq!(third.show(&mut scene, 1_000_000_000), 1_800_000_000);
        third.hide(&mut scene, 3_000_000_000);
        let plan = scene.finish().unwrap();
        let first = |property: &str| {
            plan.continuous_channels
                .iter()
                .find(|channel| channel.property == property)
                .unwrap()
                .events
                .iter()
                .map(TrackEventPlan::at_nanos)
                .collect::<Vec<_>>()
        };
        assert!(first("bar")[0] < first("name")[0] && first("name")[0] < first("role")[0]);
        assert!(first("role")[1] < first("name")[1] && first("name")[1] < first("bar")[1]);
        assert!(
            plan.continuous_channels
                .iter()
                .all(|c| LOWER_THIRD_PROPERTIES.contains(&c.property.as_str()))
        );
    }
}

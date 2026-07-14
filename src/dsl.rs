use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::timeline::{Animation, PropertyId, SpringProfile, Timeline};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextTarget {
    pub line_id: String,
    pub text: String,
}

impl TextTarget {
    pub fn new(line_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            line_id: line_id.into(),
            text: text.into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TargetGeometry {
    pub x: f32,
    pub width: f32,
    pub line_y: f32,
}

impl TargetGeometry {
    pub fn center_x(self) -> f32 {
        self.x + self.width * 0.5
    }

    pub fn below(self, offset: f32) -> f32 {
        self.line_y + offset
    }
}

#[derive(Clone, Debug)]
pub enum Scalar {
    Literal(f32),
    TargetX(TextTarget),
    TargetWidth(TextTarget),
    TargetCenterX(TextTarget),
    TargetBelow { target: TextTarget, offset: f32 },
    Offset { value: Box<Scalar>, amount: f32 },
}

impl From<f32> for Scalar {
    fn from(value: f32) -> Self {
        Self::Literal(value)
    }
}

impl Scalar {
    pub fn offset(self, amount: f32) -> Self {
        Self::Offset {
            value: Box::new(self),
            amount,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Motion {
    Set {
        property: PropertyId,
        value: Scalar,
    },
    Spring {
        property: PropertyId,
        target: Scalar,
        profile: SpringProfile,
    },
    Sequence(Vec<Motion>),
    Parallel(Vec<Motion>),
    Delay {
        seconds: f32,
        motion: Box<Motion>,
    },
    Hold(f32),
}

impl Motion {
    pub fn set(property: PropertyId, value: impl Into<Scalar>) -> Self {
        Self::Set {
            property,
            value: value.into(),
        }
    }

    pub fn spring(property: PropertyId, target: impl Into<Scalar>, profile: SpringProfile) -> Self {
        Self::Spring {
            property,
            target: target.into(),
            profile,
        }
    }

    pub fn sequence(motions: impl IntoIterator<Item = Motion>) -> Self {
        Self::Sequence(motions.into_iter().collect())
    }

    pub fn parallel(motions: impl IntoIterator<Item = Motion>) -> Self {
        Self::Parallel(motions.into_iter().collect())
    }

    pub fn delay(seconds: f32, motion: Motion) -> Self {
        Self::Delay {
            seconds,
            motion: Box::new(motion),
        }
    }

    pub fn hold(seconds: f32) -> Self {
        Self::Hold(seconds)
    }

    fn resolve(&self, targets: &HashMap<TextTarget, TargetGeometry>) -> Result<Animation> {
        Ok(match self {
            Self::Set { property, value } => {
                Animation::set(property.clone(), resolve_scalar(value, targets)?)
            }
            Self::Spring {
                property,
                target,
                profile,
            } => Animation::spring(property.clone(), resolve_scalar(target, targets)?, *profile),
            Self::Sequence(motions) => Animation::sequence(
                motions
                    .iter()
                    .map(|motion| motion.resolve(targets))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Parallel(motions) => Animation::parallel(
                motions
                    .iter()
                    .map(|motion| motion.resolve(targets))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Delay { seconds, motion } => Animation::delay(*seconds, motion.resolve(targets)?),
            Self::Hold(seconds) => Animation::hold(*seconds),
        })
    }
}

pub struct Scene {
    initial_values: Vec<(PropertyId, Scalar)>,
    animation: Motion,
}

impl Scene {
    pub fn new(
        initial_values: impl IntoIterator<Item = (PropertyId, Scalar)>,
        animation: Motion,
    ) -> Self {
        Self {
            initial_values: initial_values.into_iter().collect(),
            animation,
        }
    }

    pub fn compile(&self, targets: &HashMap<TextTarget, TargetGeometry>) -> Result<Timeline> {
        let initial_values = self
            .initial_values
            .iter()
            .map(|(property, value)| Ok((property.clone(), resolve_scalar(value, targets)?)))
            .collect::<Result<Vec<_>>>()?;
        Timeline::compile(initial_values, &self.animation.resolve(targets)?)
    }
}

#[derive(Clone)]
pub struct Pointer {
    pub x: PropertyId,
    pub y: PropertyId,
    pub opacity: PropertyId,
    pub scale: PropertyId,
    pub blur: PropertyId,
}

#[derive(Clone)]
pub struct Code {
    pub panel_y: PropertyId,
    pub layout: PropertyId,
    pub content: PropertyId,
    pub focus: PropertyId,
    pub highlight_x: PropertyId,
    pub highlight_width: PropertyId,
    pub highlight_opacity: PropertyId,
    pub inline_reveal: PropertyId,
}

impl Code {
    pub fn new(id: &str) -> Self {
        Self {
            panel_y: PropertyId::new(format!("{id}.panel_y")),
            layout: PropertyId::new(format!("{id}.layout")),
            content: PropertyId::new(format!("{id}.content")),
            focus: PropertyId::new(format!("{id}.focus")),
            highlight_x: PropertyId::new(format!("{id}.highlight.x")),
            highlight_width: PropertyId::new(format!("{id}.highlight.width")),
            highlight_opacity: PropertyId::new(format!("{id}.highlight.opacity")),
            inline_reveal: PropertyId::new(format!("{id}.inline_reveal")),
        }
    }

    pub fn text(&self, line_id: impl Into<String>, text: impl Into<String>) -> TextTarget {
        TextTarget::new(line_id, text)
    }

    pub fn highlight(&self, target: TextTarget, profile: SpringProfile) -> Motion {
        Motion::parallel([
            Motion::spring(
                self.highlight_x.clone(),
                Scalar::TargetX(target.clone()),
                profile,
            ),
            Motion::spring(
                self.highlight_width.clone(),
                Scalar::TargetWidth(target),
                profile,
            ),
            Motion::spring(self.highlight_opacity.clone(), 1.0, profile),
        ])
    }

    pub fn reveal_inline(&self, profile: SpringProfile) -> Motion {
        Motion::spring(self.inline_reveal.clone(), 1.0, profile)
    }
}

impl Pointer {
    pub fn new(id: &str) -> Self {
        Self {
            x: PropertyId::new(format!("{id}.x")),
            y: PropertyId::new(format!("{id}.y")),
            opacity: PropertyId::new(format!("{id}.opacity")),
            scale: PropertyId::new(format!("{id}.scale")),
            blur: PropertyId::new(format!("{id}.blur")),
        }
    }

    pub fn move_to(&self, target: TextTarget, offset_y: f32, profile: SpringProfile) -> Motion {
        Motion::parallel([
            Motion::spring(
                self.x.clone(),
                Scalar::TargetCenterX(target.clone()),
                profile,
            ),
            Motion::spring(
                self.y.clone(),
                Scalar::TargetBelow {
                    target,
                    offset: offset_y,
                },
                profile,
            ),
        ])
    }
}

fn resolve_scalar(scalar: &Scalar, targets: &HashMap<TextTarget, TargetGeometry>) -> Result<f32> {
    let geometry = |target: &TextTarget| {
        targets.get(target).copied().with_context(|| {
            format!(
                "semantic target '{}:{}' was not measured",
                target.line_id, target.text
            )
        })
    };
    Ok(match scalar {
        Scalar::Literal(value) => *value,
        Scalar::TargetX(target) => geometry(target)?.x,
        Scalar::TargetWidth(target) => geometry(target)?.width,
        Scalar::TargetCenterX(target) => geometry(target)?.center_x(),
        Scalar::TargetBelow { target, offset } => geometry(target)?.below(*offset),
        Scalar::Offset { value, amount } => resolve_scalar(value, targets)? + amount,
    })
}

#[cfg(test)]
mod tests {
    use super::{Motion, Pointer, Scalar, Scene, TargetGeometry, TextTarget};
    use crate::timeline::SpringProfile;
    use std::collections::HashMap;

    #[test]
    fn semantic_pointer_motion_compiles_to_scalar_tracks() {
        let pointer = Pointer::new("pointer");
        let target = TextTarget::new("line", "Effect");
        let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
        let scene = Scene::new(
            [
                (pointer.x.clone(), Scalar::Literal(0.0)),
                (pointer.y.clone(), Scalar::Literal(0.0)),
            ],
            Motion::delay(0.2, pointer.move_to(target.clone(), 55.0, profile)),
        );
        let targets = HashMap::from([(
            target,
            TargetGeometry {
                x: 100.0,
                width: 40.0,
                line_y: 80.0,
            },
        )]);
        let timeline = scene.compile(&targets).unwrap();

        assert_eq!(timeline.sample(&pointer.x, 0.0).unwrap().position, 0.0);
        assert!((timeline.sample(&pointer.x, 2.0).unwrap().position - 120.0).abs() < 0.01);
        assert!((timeline.sample(&pointer.y, 2.0).unwrap().position - 135.0).abs() < 0.01);
    }
}

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::composition::{
    Asset, AssetKind, Composition, CueId, Duration, MediaPlacement, TimeRange,
};
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

    pub fn duration(&self) -> f32 {
        match self {
            Self::Set { .. } => 0.0,
            Self::Spring { profile, .. } => profile.advance_time(),
            Self::Sequence(motions) => motions.iter().map(Self::duration).sum(),
            Self::Parallel(motions) => motions.iter().map(Self::duration).fold(0.0, f32::max),
            Self::Delay { seconds, motion } => seconds + motion.duration(),
            Self::Hold(seconds) => *seconds,
        }
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
    composition: Composition,
    images: Vec<Image>,
}

impl Scene {
    pub fn new(
        initial_values: impl IntoIterator<Item = (PropertyId, Scalar)>,
        composition: impl Into<Composition>,
    ) -> Self {
        Self {
            initial_values: initial_values.into_iter().collect(),
            composition: composition.into(),
            images: Vec::new(),
        }
    }

    pub fn with_image(mut self, image: Image) -> Self {
        self.images.push(image);
        self
    }

    pub fn compile(&self, targets: &HashMap<TextTarget, TargetGeometry>) -> Result<CompiledScene> {
        let initial_values = self
            .initial_values
            .iter()
            .map(|(property, value)| Ok((property.clone(), resolve_scalar(value, targets)?)))
            .collect::<Result<Vec<_>>>()?;
        let lowered = self.composition.lower()?;
        let timeline = Timeline::compile_with_duration(
            initial_values,
            &lowered.motion.resolve(targets)?,
            lowered.duration.as_seconds() as f32,
        )?;
        Ok(CompiledScene {
            timeline,
            media: lowered.media,
            cues: lowered.cues,
            duration: lowered.duration,
            images: self.images.clone(),
        })
    }
}

pub struct CompiledScene {
    timeline: Timeline,
    media: Vec<MediaPlacement>,
    cues: HashMap<CueId, TimeRange>,
    duration: Duration,
    images: Vec<Image>,
}

impl CompiledScene {
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn media(&self) -> &[MediaPlacement] {
        &self.media
    }

    pub fn cue(&self, id: &str) -> Option<TimeRange> {
        self.cues
            .iter()
            .find_map(|(cue_id, range)| (cue_id.as_str() == id).then_some(*range))
    }

    pub fn cues(&self) -> impl Iterator<Item = (&CueId, TimeRange)> {
        self.cues.iter().map(|(id, range)| (id, *range))
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn images(&self) -> &[Image] {
        &self.images
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

#[derive(Clone, Debug)]
pub struct Image {
    asset: Asset,
    pub x: PropertyId,
    pub y: PropertyId,
    pub scale: PropertyId,
    pub rotation: PropertyId,
    pub opacity: PropertyId,
    pub blur: PropertyId,
}

impl Image {
    pub fn new(id: &str, asset: Asset) -> Self {
        assert_eq!(
            asset.kind(),
            AssetKind::Image,
            "image actors require an image asset"
        );
        Self {
            asset,
            x: PropertyId::new(format!("{id}.x")),
            y: PropertyId::new(format!("{id}.y")),
            scale: PropertyId::new(format!("{id}.scale")),
            rotation: PropertyId::new(format!("{id}.rotation")),
            opacity: PropertyId::new(format!("{id}.opacity")),
            blur: PropertyId::new(format!("{id}.blur")),
        }
    }

    pub fn asset(&self) -> &Asset {
        &self.asset
    }
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
    use super::{Image, Motion, Pointer, Scalar, Scene, TargetGeometry, TextTarget};
    use crate::composition::{Asset, Composition, MediaRole, Time, TimeRange};
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
        let compiled = scene.compile(&targets).unwrap();
        let timeline = compiled.timeline();

        assert_eq!(timeline.sample(&pointer.x, 0.0).unwrap().position, 0.0);
        assert!((timeline.sample(&pointer.x, 2.0).unwrap().position - 120.0).abs() < 0.01);
        assert!((timeline.sample(&pointer.y, 2.0).unwrap().position - 135.0).abs() < 0.01);
    }

    #[test]
    fn scene_compilation_preserves_scheduled_media_with_visual_motion() {
        let opacity = crate::timeline::PropertyId::new("title.opacity");
        let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
        let narration = Asset::audio("narration", "assets/narration.wav")
            .clip(TimeRange::new(Time::seconds(1.0), Time::seconds(3.0)));
        let composition = Composition::parallel([
            Composition::script(narration),
            Composition::named("title", Motion::spring(opacity.clone(), 1.0, profile)),
        ]);
        let scene = Scene::new([(opacity.clone(), Scalar::Literal(0.0))], composition);

        let compiled = scene.compile(&HashMap::new()).unwrap();

        assert_eq!(compiled.media().len(), 1);
        assert_eq!(compiled.media()[0].role(), MediaRole::Script);
        assert_eq!(compiled.duration().as_seconds(), 2.0);
        assert_eq!(compiled.timeline().duration(), 2.0);
        assert_eq!(compiled.cue("title").unwrap().start(), Time::ZERO);
        assert!(compiled.timeline().sample(&opacity, 1.0).unwrap().position > 0.99);
    }

    #[test]
    fn image_assets_compile_as_stable_animated_scene_actors() {
        let logo = Image::new("logo", Asset::image("logo", "assets/logo.svg"));
        let profile = SpringProfile::from_visual_duration(0.3, 0.0, 0.001, 0.001);
        let scene = Scene::new(
            [
                (logo.opacity.clone(), Scalar::Literal(0.0)),
                (logo.scale.clone(), Scalar::Literal(0.8)),
            ],
            Motion::parallel([
                Motion::spring(logo.opacity.clone(), 1.0, profile),
                Motion::spring(logo.scale.clone(), 1.0, profile),
            ]),
        )
        .with_image(logo.clone());

        let compiled = scene.compile(&HashMap::new()).unwrap();

        assert_eq!(compiled.images().len(), 1);
        assert_eq!(compiled.images()[0].asset().id().as_str(), "logo");
        assert!(
            compiled
                .timeline()
                .sample(&logo.opacity, 1.0)
                .unwrap()
                .position
                > 0.99
        );
    }
}

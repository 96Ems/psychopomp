//! Independently sampled content channels. Timing lives in Task preparation,
//! not a second easing/window applied to an already animated state weight.
use kinograph::dsl::TaskState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentPose {
    pub scale: f32,
    pub opacity: f32,
    pub blur: f32,
    /// Relative to the body. Only symbols receive a rotational flourish.
    pub rotation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BubblePose {
    pub content: ContentPose,
    pub y: f32,
}

pub struct TaskContentFrame<'a> {
    pub state: &'a TaskState,
    pub pose: ContentPose,
    pub bubble: Option<BubblePose>,
}

use psychopomp::{
    anchor::{AnchorPlan, Edge},
    author::{PlanBuilder, SECOND},
    footage::{self, Clip, Fit, FootageActor, FootagePlan},
    image::{self, ImageActor, ImagePlan},
    plan::{MediaKindPlan, ScenePlan},
    stage::{StageActor, StagePlan},
};

use super::{decode_width, scaled, shown};
use crate::plan_runtime::validate_renderer_plan;

fn image_plan(anchor: Option<AnchorPlan>, property: &str, kind: MediaKindPlan) -> ScenePlan {
    let mut scene = PlanBuilder::new("image", 2 * SECOND);
    let mut recipe = ImagePlan::new("shot", [960.0, 540.0], 400.0).framed();
    if let Some(anchor) = anchor {
        recipe = recipe.anchor(anchor);
    }
    let media = image::media("shot", "never-opened.png", 0, 2 * SECOND);
    let mut actor = ImageActor::declare(&mut scene, "shot", &recipe, media).unwrap();
    let channel = actor.channel(&mut scene, property, 0.0);
    scene.set(&channel, 100_000_000, 1.0);
    let mut plan = scene.finish().unwrap();
    plan.media[0].kind = kind;
    plan
}

#[test]
fn image_preflight_consumes_its_media_and_checks_channels_without_reading_it() {
    use MediaKindPlan::{Image, Video};
    validate_renderer_plan(&image_plan(None, "tilt-x", Image)).unwrap();
    validate_renderer_plan(&image_plan(
        Some(AnchorPlan::point("here", [100.0, 100.0])),
        "anchor.here",
        Image,
    ))
    .unwrap();
    for (anchor, property, kind, expected) in [
        (None, "focus-x", Image, "unknown property"),
        (None, "opacity", Video, "must reference image media"),
        (
            Some(AnchorPlan::stage("card", "card", Edge::Top)),
            "opacity",
            Image,
            "needs a stage root",
        ),
    ] {
        let error = validate_renderer_plan(&image_plan(anchor, property, kind)).unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
}

fn footage_plan(plan: FootagePlan, kind: MediaKindPlan, property: &str) -> ScenePlan {
    let mut scene = PlanBuilder::new("footage", 2 * SECOND);
    let media = footage::media("clip", "never-opened.mp4", 0, 2 * SECOND);
    let actor = FootageActor::declare(&mut scene, "clip", &plan, media).unwrap();
    let channel = actor.channel(&mut scene, property);
    scene.set(&channel, 100_000_000, 0.5);
    let mut built = scene.finish().unwrap();
    built.media[0].kind = kind;
    built
}

#[test]
fn footage_preflight_checks_channels_media_and_anchors_without_reading_files() {
    use MediaKindPlan::{Audio, Image, Video};
    let plan = FootagePlan::new(Clip::new("clip"), [960.0, 540.0], [480.0, 270.0]).circle();
    for property in ["time", "saturation", "defocus", "focus-size", "tilt-y"] {
        validate_renderer_plan(&footage_plan(plan.clone(), Video, property)).unwrap();
    }
    validate_renderer_plan(&footage_plan(plan.clone(), Image, "opacity")).unwrap();
    for (plan, kind, property, expected) in [
        (plan.clone(), Video, "z", "unknown property"),
        (
            plan.clone(),
            Audio,
            "opacity",
            "must reference video or image media",
        ),
        (
            plan.clone()
                .anchor(AnchorPlan::stage("card", "card", Edge::Top)),
            Video,
            "opacity",
            "needs a stage root",
        ),
    ] {
        let error = validate_renderer_plan(&footage_plan(plan, kind, property)).unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
}

fn stage_plan(media_kind: Option<MediaKindPlan>) -> ScenePlan {
    let recipe: StagePlan = serde_json::from_value(serde_json::json!({
        "elements": [
            { "kind": "card", "id": "card", "at": [400, 400, 0], "size": [300, 110], "title": "card" },
            { "kind": "footage", "id": "tv", "at": [1200, 540, -300], "size": [480, 270],
              "clip": { "media": "clip", "repeat": "loop" }, "framed": true }
        ]
    }))
    .unwrap();
    let mut scene = PlanBuilder::new("stage-footage", 2 * SECOND);
    let mut stage = StageActor::declare(&mut scene, "stage", &recipe).unwrap();
    stage.to(&mut scene, "tv.saturation", 0, 0.2, 0.5);
    if let Some(kind) = media_kind {
        let mut media = footage::media("clip", "never-opened.mp4", 0, 2 * SECOND);
        media.kind = kind;
        scene.media(media);
    }
    scene.finish().unwrap()
}

#[test]
fn stage_footage_must_play_a_planned_video_or_image() {
    validate_renderer_plan(&stage_plan(Some(MediaKindPlan::Video))).unwrap();
    validate_renderer_plan(&stage_plan(Some(MediaKindPlan::Image))).unwrap();
    for (kind, expected) in [
        (None, "unknown media 'clip'"),
        (
            Some(MediaKindPlan::Audio),
            "must reference video or image media",
        ),
    ] {
        let error = validate_renderer_plan(&stage_plan(kind)).unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
}

#[test]
fn decodes_are_about_twice_the_widest_showing_on_a_shared_ladder() {
    // A 480 px tile of a 1920 px recording decodes at 960 wide.
    assert_eq!(
        decode_width([1920, 1080], Fit::Cover, [480.0, 270.0], 480.0),
        960
    );
    // Cropped to a square, the visible window is narrower: more pixels.
    assert_eq!(
        decode_width([1920, 1080], Fit::Cover, [300.0, 300.0], 300.0),
        1280
    );
    // Never past the source.
    assert_eq!(
        decode_width([640, 360], Fit::Fill, [1200.0, 675.0], 1200.0),
        640
    );
    // Nearby tile sizes share a rung, so they share a decode.
    assert_eq!(
        decode_width([1920, 1080], Fit::Fill, [300.0, 169.0], 300.0),
        decode_width([1920, 1080], Fit::Fill, [310.0, 174.0], 310.0)
    );
    assert_eq!(scaled([1920, 1080], 960), [960, 540]);
    assert_eq!(scaled([1200, 721], 241), [240, 144]);

    let mut scene = PlanBuilder::new("shown", 2 * SECOND);
    let plan = FootagePlan::new(Clip::new("clip"), [960.0, 540.0], [400.0, 225.0]);
    let actor = FootageActor::declare(
        &mut scene,
        "clip",
        &plan,
        footage::media("clip", "clip.mp4", 0, 2 * SECOND),
    )
    .unwrap();
    actor.to(&mut scene, "scale", 0, 1.5, 0.4);
    actor.focus(&mut scene, SECOND, [0.25, 0.25, 0.5, 0.5], 0.5);
    let built = scene.finish().unwrap();
    assert_eq!(
        shown(&built.continuous_channels, "clip", "", 400.0),
        400.0 * 1.5 / 0.5
    );
}

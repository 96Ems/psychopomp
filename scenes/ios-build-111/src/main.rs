//! What changed in the OpenCode iPhone app over a few days, up to TestFlight
//! build 111: one segment per change, each a phone recording from the app's
//! UI tests beside the points the narration makes. Every recording is
//! retimed so the moment it shows lands on the phrase that names it.
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{PlanBuilder, millis, seconds},
    caption::{CaptionActor, CaptionPlan, CaptionSpanPlan},
    chrome::header,
    footage::{Clip, FootageActor, FootagePlan},
    narration::Reading,
    plan::{ReelPlan, ScenePlan},
    sfx,
    tone::Tone,
};
use psychopomp_media::{Audio, Media, Voice};

/// Kit's ElevenLabs Professional Voice Clone.
const KIT: &str = "8olojUk4IXpvKgaOCHXj";

/// Simulator recordings are 1320 × 2868.
const PHONE_ASPECT: f32 = 1320.0 / 2868.0;
const PHONE_HEIGHT: f32 = 1000.0;
const PHONE_CENTER: [f32; 2] = [1390.0, 540.0];
const BULLET_X: f32 = 140.0;
const BULLET_TOP: f32 = 330.0;
const BULLET_PITCH: f32 = 100.0;

/// One change: what is said, the shots that show it, and its points.
struct Segment {
    id: &'static str,
    title: &'static str,
    narration: &'static str,
    /// `(phrase, recording, from, to)`: when the phrase is spoken, cut to
    /// `from` seconds into the recording and play to `to`, then hold. `""`
    /// is the segment's start. The shots are cut into `footage/<id>.mp4`.
    shots: &'static [(&'static str, &'static str, f32, f32)],
    /// `(phrase, text)`: a point appears on the left when it is spoken.
    bullets: &'static [(&'static str, &'static str)],
    /// `(phrase, [x, y, w, h])`: zoom into a region (fractions of the frame);
    /// `[0, 0, 1, 1]` returns to the whole screen.
    focus: &'static [(&'static str, [f32; 4])],
}

const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

const SEGMENTS: &[Segment] = &[
    Segment {
        id: "intro",
        title: "build 111",
        narration: "Here's what changed in the OpenCode iPhone app over the last few days. Everything you'll see is in TestFlight build one eleven.",
        shots: &[("", "testLongFormMarkdownTypography", 14.0, 27.0)],
        bullets: &[],
        focus: &[],
    },
    Segment {
        id: "header",
        title: "session header",
        narration: "The session header is simpler. The title sits on the left, with the project underneath. A new more button holds every session action: pin, rename, compact, fork, and export. And its rim is the context gauge. It fills as the context window does.",
        shots: &[
            ("", "testHeaderMenuActsWithoutTheSheet", 33.5, 36.2),
            (
                "more button",
                "testHeaderMenuActsWithoutTheSheet",
                36.3,
                41.5,
            ),
            ("its rim", "testHeaderMenuActsWithoutTheSheet", 33.5, 36.2),
        ],
        bullets: &[
            ("title sits", "title and project on the left"),
            ("more button", "every session action in one menu"),
            ("its rim", "the rim fills with context"),
        ],
        focus: &[
            ("title sits", [0.0, 0.0, 0.5, 0.5]),
            ("more button", WHOLE),
            ("its rim", [0.62, 0.0, 0.38, 0.14]),
        ],
    },
    Segment {
        id: "badge",
        title: "drawer dot",
        narration: "A dot on the drawer button means another session wants you. Blue when it's unread. Orange when it needs your input.",
        shots: &[
            ("", "testDrawerButtonBadgesSessionsElsewhere", 11.0, 12.6),
            (
                "Orange",
                "testDrawerButtonBadgesSessionsElsewhere",
                12.6,
                13.4,
            ),
        ],
        bullets: &[
            ("Blue", "blue: another session is unread"),
            ("Orange", "orange: one needs your input"),
        ],
        focus: &[("dot", [0.0, 0.03, 0.42, 0.1])],
    },
    Segment {
        id: "sheet",
        title: "session sheet",
        narration: "Tap the title for the session sheet. It has the model, how much context is used, and what the session has cost, subagents included.",
        shots: &[("", "testTitleAndMenuOpenTheSessionSheet", 32.0, 41.0)],
        bullets: &[
            ("model", "model, context, and cost"),
            ("subagents", "cost includes subagents"),
        ],
        focus: &[],
    },
    Segment {
        id: "markdown",
        title: "replies",
        narration: "Replies read better. Headings, paragraphs and lists are spaced like a document, and nested lists indent. Code blocks and tables run edge to edge, and scroll sideways instead of wrapping.",
        shots: &[
            ("", "testLongFormMarkdownTypography", 14.0, 27.0),
            (
                "Code blocks",
                "testTableScrollsUnderFullWidthRules",
                46.5,
                56.0,
            ),
        ],
        bullets: &[
            ("Headings", "document spacing and nested lists"),
            ("Code blocks", "code and tables run edge to edge"),
            ("sideways", "and scroll instead of wrapping"),
        ],
        focus: &[],
    },
    Segment {
        id: "copy",
        title: "copy a reply",
        narration: "Every finished reply ends with a copy button. It copies the whole reply as Markdown, with code and tables intact.",
        shots: &[("", "testFinishedReplyOffersCopy", 20.0, 24.8)],
        bullets: &[
            ("copy button", "a copy button under each reply"),
            ("whole reply", "copies the whole reply as markdown"),
        ],
        focus: &[("copy button", [0.0, 0.62, 0.75, 0.3])],
    },
    Segment {
        id: "steering",
        title: "steer or queue",
        narration: "Sending while the agent works steers it by default. The message waits in the transcript, then lands in place. Hold Send to queue it instead. Queued messages wait in a pill, where you can send them now, or cancel.",
        shots: &[
            ("", "testSteerByDefaultAndQueueFromSendMenu", 21.0, 27.3),
            (
                "Hold Send",
                "testSteerByDefaultAndQueueFromSendMenu",
                44.5,
                50.5,
            ),
            (
                "Queued messages",
                "testQueuedMessageCanBeSentNowOrCancelled",
                43.0,
                50.0,
            ),
        ],
        bullets: &[
            ("steers", "send steers by default"),
            ("Hold Send", "hold send to queue"),
            ("send them now", "send now or cancel a queued one"),
        ],
        focus: &[],
    },
    Segment {
        id: "background",
        title: "background work",
        narration: "Background work sits above the composer: one subagent, two shells. Tap it to see each one, and open a shell to watch its output live. When a command blocks the agent, Move to Background sets it free.",
        shots: &[
            (
                "",
                "testBackgroundShellsJoinSubagentsInTheActivityCapsule",
                11.5,
                19.5,
            ),
            (
                "open a shell",
                "testShellOutputTailsLiveAndStopsAfterConfirming",
                19.5,
                29.5,
            ),
            (
                "blocks",
                "testMoveToBackgroundAppearsAfterABlockingShellAndMovesIt",
                14.5,
                21.0,
            ),
        ],
        bullets: &[
            ("composer", "subagents and shells in one pill"),
            ("open a shell", "live shell output"),
            ("Move to Background", "move a blocking command aside"),
        ],
        focus: &[],
    },
    Segment {
        id: "attachments",
        title: "attachments",
        narration: "The plus button grows into a menu: camera, photos, and files. Photos opens the picker right under the composer, and one tap attaches a photo.",
        shots: &[
            ("", "testAttachmentMenuMorphsIntoInlinePicker", 51.5, 58.0),
            (
                "Photos opens",
                "testAttachmentMenuMorphsIntoInlinePicker",
                62.5,
                70.0,
            ),
            (
                "one tap",
                "testAttachmentMenuMorphsIntoInlinePicker",
                74.5,
                80.0,
            ),
        ],
        bullets: &[
            ("plus button", "camera, photos, and files"),
            ("Photos opens", "an inline photo picker"),
            ("one tap", "one tap attaches"),
        ],
        focus: &[],
    },
    Segment {
        id: "files",
        title: "files",
        narration: "When the agent names a file, tap it. Code, PDFs, video, audio, and Markdown all open in native viewers.",
        shots: &[
            ("", "testAgentFilesOpenInNativeViewers", 104.5, 107.0),
            ("Code", "testAgentFilesOpenInNativeViewers", 107.5, 112.0),
            ("PDFs", "testAgentFilesOpenInNativeViewers", 116.5, 120.0),
            ("video", "testAgentFilesOpenInNativeViewers", 129.5, 133.0),
            ("audio", "testAgentFilesOpenInNativeViewers", 143.5, 147.0),
            (
                "all open",
                "testAgentFilesOpenInNativeViewers",
                157.5,
                164.0,
            ),
        ],
        bullets: &[
            ("tap it", "tap a file the agent names"),
            ("native viewers", "it opens in a native viewer"),
        ],
        focus: &[],
    },
    Segment {
        id: "pins",
        title: "pinned sessions",
        narration: "And in the drawer, hold a pinned session and drag it to reorder.",
        shots: &[(
            "",
            "testPinnedRowsReorderByLongPressDragAndKeepContextMenu",
            11.5,
            20.0,
        )],
        bullets: &[("drag", "drag pinned sessions to reorder")],
        focus: &[],
    },
    Segment {
        id: "outro",
        title: "build 111",
        narration: "That's build one eleven. It's on TestFlight now. Tell me what feels off.",
        shots: &[("", "testHeaderMenuActsWithoutTheSheet", 33.5, 36.2)],
        bullets: &[("now", "on testflight now")],
        focus: &[],
    },
];

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if std::env::args().nth(1).as_deref() == Some("--shots") {
        // For scripts/cut.py, which cuts footage/<id>.mp4 from the recordings.
        let shots = SEGMENTS
            .iter()
            .map(|segment| {
                let shots = segment
                    .shots
                    .iter()
                    .map(|(_, recording, from, to)| serde_json::json!([recording, from, to]))
                    .collect::<Vec<_>>();
                (segment.id.to_owned(), serde_json::Value::Array(shots))
            })
            .collect::<serde_json::Map<_, _>>();
        println!("{}", serde_json::to_string_pretty(&shots)?);
        return Ok(());
    }
    let media = Media::open(&root)?;
    let kit = Voice::eleven(KIT)
        .v4()
        .stability(0.2)
        .similarity(0.65)
        .whisper();
    let lines = SEGMENTS
        .iter()
        .map(|segment| {
            let line = format!("[relaxed, conversational, clear] {}", segment.narration);
            media.dialogue(segment.id, [(&kit, line.as_str())])
        })
        .collect::<Result<Vec<_>>>()?;
    media.finish()?;

    let plans = SEGMENTS
        .iter()
        .zip(&lines)
        .map(|(segment, line)| build(segment, line).with_context(|| segment.id))
        .collect::<Result<Vec<_>>>()?;
    let reel = ReelPlan::dipped("ios-build-111", plans, millis(600))?;
    reel.validate()?;
    let output = root.join("ios-build-111.reel.json");
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    eprintln!(
        "wrote {} ({:.1}s)",
        output.display(),
        reel.duration_nanos() as f64 / 1e9
    );
    Ok(())
}

fn build(segment: &Segment, line: &Audio) -> Result<ScenePlan> {
    let reading = Reading::new(seconds(0.7), [(line.clip(), seconds(1.0))]);
    let mut scene = PlanBuilder::new(segment.id, reading.duration());
    let [spoken] = reading.place(&mut scene);
    let at = |phrase: &str| {
        if phrase.is_empty() {
            0
        } else {
            spoken.at(phrase)
        }
    };

    let mut title = header(&mut scene, "opencode ios", segment.title)?;
    title.type_in(&mut scene, millis(150), 52.0, 0.6);

    let size = [PHONE_HEIGHT * PHONE_ASPECT, PHONE_HEIGHT];
    let clip = Clip::new("phone").resolution(900).decoded_at(30);
    let media = psychopomp::footage::media(
        "phone",
        format!("footage/{}.mp4", segment.id),
        0,
        scene.duration_nanos(),
    );
    let phone = FootageActor::declare(
        &mut scene,
        "phone",
        &FootagePlan::new(clip, PHONE_CENTER, size).rounded(64.0),
        media,
    )?;
    phone.fly_in(&mut scene, millis(100));

    // Each shot cuts in on its phrase and holds its last frame until the next.
    let playhead = phone.playhead().clone();
    let starts = segment
        .shots
        .iter()
        .map(|(phrase, ..)| at(phrase))
        .collect::<Vec<_>>();
    let mut offset = 0.0;
    for (index, (_, _, from, to)) in segment.shots.iter().enumerate() {
        let start = starts[index];
        let length = to - from;
        playhead.seek(&mut scene, start, offset);
        playhead.play(&mut scene, start, 1.0);
        let end = start + millis(u64::from((length * 1000.0) as u32));
        let next = starts
            .get(index + 1)
            .copied()
            .unwrap_or(scene.duration_nanos());
        if end < next {
            playhead.freeze(&mut scene, end);
        }
        offset += length;
    }

    for (phrase, region) in segment.focus {
        if *region == [0.0, 0.0, 1.0, 1.0] {
            phone.unfocus(&mut scene, at(phrase), 0.8);
        } else {
            phone.focus(&mut scene, at(phrase), *region, 0.8);
        }
    }

    for (index, (phrase, text)) in segment.bullets.iter().enumerate() {
        let mut bullet = CaptionActor::declare(
            &mut scene,
            format!("bullet-{index}"),
            &CaptionPlan::line(
                [BULLET_X, BULLET_TOP + index as f32 * BULLET_PITCH],
                38.0,
                vec![
                    CaptionSpanPlan::new("› ", Tone::Accent),
                    CaptionSpanPlan::new(*text, Tone::Plain),
                ],
            ),
        )?;
        let when = at(phrase);
        bullet.type_in(&mut scene, when, 60.0, 0.3);
        sfx::TICK.play(&mut scene, format!("tick-{index}"), when, -20.0);
    }

    scene.cue(segment.id, 0, reading.duration());
    scene.finish().context("scene")
}

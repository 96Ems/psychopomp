//! High Council: a Civilization II-style FMV advisory council that pitches
//! OpenCode improvements every morning.
//!
//! The council segments are a 1996 CD-ROM screen: pre-generated portraits
//! played as Sprite Sheets (`psychopomp::lipsync`), their mouths cut on the
//! sprite tick from each designed voice's character alignment. The chrome,
//! speech box, agenda, and shout banners are image sequences rendered by
//! `scripts/frames.py` from the `frames.json` this program writes, so every
//! swap stays on the narration clock. The middle segment explains the
//! pipeline on the Stage.
//!
//! ```sh
//! cargo run -p psychopomp-high-council
//! python3 scenes/high-council/scripts/frames.py
//! cargo run --release -- plan render scenes/high-council/high-council.reel.json output/high-council.mp4
//! ```
use std::{collections::BTreeMap, fs, path::PathBuf};

use anyhow::{Context, Result};
use psychopomp::{
    author::{MILLISECOND, PlanBuilder, SECOND, millis, seconds},
    footage::{self, Clip, FootageActor, FootagePlan},
    lipsync::{self, Mouth, Sprite, SpriteSheet, TICK},
    math::random::smooth_noise,
    plan::{
        MediaKindPlan, MediaPlan, MediaRolePlan, ReelPlan, ReelSegmentPlan, ReelTransitionStyle,
        ScenePlan,
    },
    sfx,
    stage::{StageActor, StageElement, StagePlan, StagePost},
    tone::Tone,
};
use psychopomp_media::{Audio, Effect, Media, Sound, Voice};
use serde_json::{Value, json};

const ADVISORS: [(&str, &str); 6] = [
    ("simplifier", "Simplifier"),
    ("performance", "Performance"),
    ("bug-hunter", "Bug Hunter"),
    ("architect", "Architect"),
    ("user-voice", "User Voice"),
    ("contrarian", "Contrarian"),
];
const EXPRESSIONS: [&str; 5] = ["neutral", "excited", "worried", "smug", "annoyed"];
/// Static frames appended after each Sprite Sheet, for the boot flicker.
const STATIC_FRAMES: u32 = 3;
const SPRITE_FPS: u32 = 1;

const ALL: &[&str] = &[
    "simplifier",
    "performance",
    "bug-hunter",
    "architect",
    "user-voice",
    "contrarian",
];
const CHORUS: &[&str] = &[
    "simplifier",
    "performance",
    "bug-hunter",
    "architect",
    "user-voice",
];

/// The 1920×1080 council screen. Panels are 4:3 wells in a 3×2 grid.
const SPRITE: [f32; 2] = [440.0, 330.0];
const BEZEL: [f32; 2] = [468.0, 358.0];
const COLUMNS: [f32; 3] = [276.0, 764.0, 1252.0];
const ROWS: [f32; 2] = [270.0, 668.0];
const SPEECH: ([f32; 2], [f32; 2]) = ([960.0, 973.0], [1840.0, 170.0]);
const AGENDA: ([f32; 2], [f32; 2]) = ([1688.0, 469.0], [384.0, 770.0]);
const BANNER: [f32; 2] = [560.0, 92.0];

struct Part {
    id: &'static str,
    speakers: &'static [&'static str],
    expression: &'static str,
    text: &'static str,
    /// Silence before this part, in milliseconds; negative interrupts.
    gap: i64,
    banner: Option<&'static str>,
    /// A change of face mid-line: at the word, the expression.
    turn: Option<(&'static str, &'static str)>,
    act: usize,
}

const fn part(
    id: &'static str,
    speakers: &'static [&'static str],
    expression: &'static str,
    gap: i64,
    act: usize,
    text: &'static str,
) -> Part {
    Part {
        id,
        speakers,
        expression,
        text,
        gap,
        banner: None,
        turn: None,
        act,
    }
}

const fn turn(mut part: Part, word: &'static str, expression: &'static str) -> Part {
    part.turn = Some((word, expression));
    part
}

const fn shout(mut part: Part, banner: &'static str) -> Part {
    part.banner = Some(banner);
    part
}

const COUNCIL: &[Part] = &[
    part(
        "n-hook",
        &["narrator"],
        "",
        0,
        0,
        "[warm, amused, conversational] Imagine waking up to this.",
    ),
    part(
        "all-hello",
        CHORUS,
        "excited",
        2600,
        0,
        "[booming, theatrical, shouting together] GOOD MORNING, SIRE! The High Council demands an audience!",
    ),
    shout(
        part(
            "contrarian-hello",
            &["contrarian"],
            "smug",
            250,
            0,
            "[sly, drawling, unimpressed] Sip your coffee slowly, Sire. Let these maniacs loose without me, and we'll be in Anarchy before breakfast.",
        ),
        "THE CONTRARIAN",
    ),
    part(
        "n-setup",
        &["narrator"],
        "",
        450,
        0,
        "[warm, conversational] Every morning, six AI advisors read the OpenCode repo and its issues while you sleep. Each one pitches a single idea... [amused] and then they argue about it.",
    ),
    // Act I: the Simplifier.
    turn(
        part(
            "simplifier-1",
            &["simplifier"],
            "excited",
            600,
            1,
            "[imperious, thrilled] SIRE! Deep in the GitHub Copilot dungeon sits a twenty-five-file, four-thousand-line mummy! A vendored fork of the Vercel AI SDK, just to speak chat and responses!",
        ),
        "mummy",
        "annoyed",
    ),
    turn(
        part(
            "simplifier-2",
            &["simplifier"],
            "smug",
            250,
            1,
            "[gleeful, savoring every word] Our own native AI package already speaks both. Give me the cleaver, and I shall lop off four thousand lines before noon!",
        ),
        "cleaver",
        "excited",
    ),
    shout(
        part(
            "architect-1",
            &["architect"],
            "excited",
            150,
            1,
            "[pompous, delighted] Hear her, Noble Sovereign! One native pipeline for every Copilot route, instead of translating every prompt twice!",
        ),
        "ONE PIPELINE!",
    ),
    shout(
        turn(
            part(
                "contrarian-1",
                &["contrarian"],
                "annoyed",
                -350,
                1,
                "[interrupting, quick and sneering] I DISAGREE, SIRE! That ugly fork round-trips Copilot's reasoning token across tool turns! Teach the native path that trick first, or the cleaver lobotomizes every session!",
            ),
            "teach",
            "smug",
        ),
        "I DISAGREE, SIRE!",
    ),
    // Act II: Performance.
    turn(
        part(
            "performance-1",
            &["performance"],
            "worried",
            600,
            2,
            "[frantic, breathless, very fast] My Liege! Put your ear to the motherboard! One agent step with ten tool calls fires forty-seven durable events!",
        ),
        "fortyseven",
        "excited",
    ),
    turn(
        part(
            "performance-2",
            &["performance"],
            "annoyed",
            200,
            2,
            "[manic, rapid-fire] And every single one opens its own SQLite transaction, reads the whole message, decodes it, and rewrites the entire row! Keep the hot message in memory, Sire!",
        ),
        "keep",
        "worried",
    ),
    shout(
        part(
            "user-voice-1",
            &["user-voice"],
            "worried",
            150,
            2,
            "[alarmed, projecting to a crowd] The citizens are revolting, Sire! In issue thirty-three three fifty-six, their database has swollen past thirteen gigabytes!",
        ),
        "THE CITIZENS REVOLT!",
    ),
    shout(
        part(
            "contrarian-2",
            &["contrarian"],
            "annoyed",
            -300,
            2,
            "[quick, dismissive, dry] Calm your stopwatch, Baron! Cache the decoded rows, fine. But buffer durable tool calls in RAM, and crash recovery goes blind!",
        ),
        "CALM YOUR STOPWATCH!",
    ),
    // Act III: the Bug Hunter.
    turn(
        part(
            "bug-hunter-2",
            &["bug-hunter"],
            "smug",
            600,
            3,
            "[gravelly, grim relish] Three prize beetles, Sire, caught alive in the compaction chamber! [barking, intense] Beetle the First! We ask the model for a summary, but hand it every tool. So it calls a tool instead, and compaction dies screaming: no summary!",
        ),
        "beetle",
        "excited",
    ),
    turn(
        part(
            "bug-hunter-3",
            &["bug-hunter"],
            "excited",
            200,
            3,
            "[urgent, gravelly] Beetle the Second drags two hundred thousand tokens past the keep budget! Beetle the Third stuffs the model's reasoning into the recap! Let me crush all three today!",
        ),
        "crush",
        "smug",
    ),
    shout(
        part(
            "user-voice-2",
            &["user-voice"],
            "excited",
            150,
            3,
            "[pleading, passionate] Two hundred and sixty-eight open issues mention compaction, Sire!",
        ),
        "268 OPEN ISSUES!",
    ),
    shout(
        part(
            "contrarian-3",
            &["contrarian"],
            "smug",
            -250,
            3,
            "[quick, silky, condescending] Hold your magnifying glass, Mandible! Set tool choice to none first. Rip out the tools, and you torch the prompt cache!",
        ),
        "HOLD IT, MANDIBLE!",
    ),
];

const CLOSER: &[Part] = &[
    part(
        "architect-close",
        &["architect"],
        "neutral",
        900,
        4,
        "[grand, ceremonial] Three royal decrees await your seal, Sire.",
    ),
    shout(
        turn(
            part(
                "contrarian-close",
                &["contrarian"],
                "smug",
                250,
                4,
                "[smirking, theatrical] Or press Anarchy, and let the issue tracker burn. I'm paid either way.",
            ),
            "paid",
            "excited",
        ),
        "ANARCHY!",
    ),
    part(
        "all-close",
        ALL,
        "excited",
        350,
        4,
        "[all shouting together, eager] Which decree shall we ship today, Sire?!",
    ),
    part(
        "n-outro",
        &["narrator"],
        "",
        700,
        4,
        "[warm, wry] That's the High Council. Good morning.",
    ),
];

const HOW: &str = "[clear, friendly, explaining] Here's how it works. Every morning, a schedule wakes the council. Each advisor is a sub-agent with one job. It researches the OpenCode repo and its open issues, and comes back with one pitch, evidence attached.";
const VOICES: &str = "[amused, warm] Then every pitch gets its own designed voice. The speech comes back with timestamps, which pick a mouth shape on pre-generated portraits. No AI video. Just sprites, swapped on the audio clock, like it's nineteen ninety-six.";

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let voices: BTreeMap<String, Value> =
        serde_json::from_str(&fs::read_to_string(root.join("narration/voices.json"))?)?;
    let voice = |id: &str| -> Result<Voice> {
        let id = voices[id]["voiceId"].as_str().context("voice id")?;
        let voice = Voice::eleven(id).v4();
        Ok(if id == voices["narrator"]["voiceId"] {
            voice.stability(0.5).similarity(0.75)
        } else {
            voice.stability(0.3).similarity(0.75)
        })
    };
    let media = Media::open(&root)?;
    let mut lines = BTreeMap::new();
    for part in COUNCIL.iter().chain(CLOSER) {
        let mut audios = Vec::new();
        for speaker in part.speakers {
            let id = if part.speakers.len() > 1 {
                format!("{}-{speaker}", part.id)
            } else {
                part.id.to_owned()
            };
            let audio = media.say(&id, &voice(speaker)?, part.text)?;
            // Advisors talk a touch faster than they were directed; pitch holds.
            // The Contrarian drawls most, so he gets the biggest nudge.
            audios.push(match *speaker {
                "narrator" => audio,
                "contrarian" => audio.derive(Effect::tempo(1.2))?,
                _ => audio.derive(Effect::tempo(1.12))?,
            });
        }
        lines.insert(part.id, audios);
    }
    let how = media.say("n-how", &voice("narrator")?, HOW)?;
    let voiced = media.say("n-voices", &voice("narrator")?, VOICES)?;
    let sounds = Sounds {
        boot: media.sfx("boot", Sound::new("A 1990s PC CD-ROM drive spinning up: motor whir rising, a mechanical click and seek chatter, then a soft hum. No music, no speech.").influence(0.6), seconds(2.5))?,
        fanfare: media.sfx("fanfare", Sound::new("A short regal fanfare played on a 1990s General MIDI sound card: cheesy synth trumpets and timpani, triumphant medieval court flourish, lo-fi FM synthesis. No vocals.").influence(0.6), seconds(3.0))?,
        sting: media.sfx("sting", Sound::new("A single dramatic orchestra hit stab from a 1990s MIDI sound card, cheesy and abrupt, short decay. No vocals.").influence(0.7), seconds(1.0))?,
        bed: media.sfx("bed", Sound::new("Lo-fi 1990s General MIDI background music for a medieval strategy game throne room: gentle harpsichord, soft synth strings and recorder, calm and regal, seamless loop. No vocals, no percussion hits.").looping(), seconds(20.0))?,
    };
    media.finish()?;

    let mut frames = Frames::default();
    let council = council_film("council", COUNCIL, &lines, &sounds, &mut frames, true)?;
    let stage = how_it_works(&how, &voiced)?;
    let closer = council_film("closer", CLOSER, &lines, &sounds, &mut frames, false)?;
    let reel = ReelPlan {
        version: ReelPlan::VERSION,
        id: "high-council".to_owned(),
        segments: vec![
            ReelSegmentPlan::new(council, 0, ReelTransitionStyle::Dip),
            ReelSegmentPlan::new(stage, millis(700), ReelTransitionStyle::Glitch),
            ReelSegmentPlan::new(closer, millis(700), ReelTransitionStyle::Glitch),
        ],
    };
    reel.validate()?;
    let output = root.join("high-council.reel.json");
    fs::write(&output, serde_json::to_string_pretty(&reel)? + "\n")?;
    let spec = root.join("../../output/high-council/frames.json");
    fs::create_dir_all(spec.parent().unwrap())?;
    fs::write(&spec, serde_json::to_string_pretty(&frames.json())? + "\n")?;
    eprintln!(
        "wrote {} ({:.1}s) and {}",
        output.display(),
        reel.duration_nanos() as f64 / 1e9,
        spec.display()
    );
    Ok(())
}

struct Sounds {
    boot: Audio,
    fanfare: Audio,
    sting: Audio,
    bed: Audio,
}

/// What `scripts/frames.py` must draw, frame by frame.
#[derive(Default)]
struct Frames {
    /// Speech box pages; frame 0 is the empty box.
    speech: Vec<Value>,
    banners: Vec<String>,
}

impl Frames {
    fn json(&self) -> Value {
        let panels: Vec<Value> = ADVISORS
            .iter()
            .enumerate()
            .map(|(index, (slug, label))| {
                let [x, y] = panel_center(index);
                json!({ "slug": slug, "label": label, "center": [x, y] })
            })
            .collect();
        json!({
            "sprite": SPRITE,
            "bezel": BEZEL,
            "speech": { "center": SPEECH.0, "size": SPEECH.1, "pages": self.speech },
            "agenda": { "center": AGENDA.0, "size": AGENDA.1 },
            "banner": { "size": BANNER, "texts": self.banners },
            "expressions": EXPRESSIONS,
            "panels": panels,
        })
    }

    fn page(&mut self, label: &str, words: &[String], highlight: Option<usize>) -> u32 {
        self.speech
            .push(json!({ "label": label, "words": words, "highlight": highlight }));
        self.speech.len() as u32
    }

    fn banner(&mut self, text: &str) -> u32 {
        self.banners.push(text.to_owned());
        self.banners.len() as u32
    }
}

fn panel_center(index: usize) -> [f32; 2] {
    [COLUMNS[index % 3], ROWS[index / 3]]
}

fn advisor_index(slug: &str) -> Option<usize> {
    ADVISORS.iter().position(|(id, _)| *id == slug)
}

fn label_of(speakers: &[&str]) -> &'static str {
    match speakers {
        [one] => advisor_index(one).map_or("Narrator", |index| ADVISORS[index].1),
        _ => "All Advisors",
    }
}

fn seq(path: &str) -> String {
    format!("../../output/high-council/{path}")
}

/// A frame-cut image sequence overlay: hidden until shown, never playing.
fn sequence(
    scene: &mut PlanBuilder,
    id: &str,
    path: &str,
    center: [f32; 2],
    size: [f32; 2],
    resolution: u32,
) -> Result<FootageActor> {
    let clip = Clip::new(id).decoded_at(SPRITE_FPS).resolution(resolution);
    let actor = FootageActor::declare(
        scene,
        id,
        &FootagePlan::new(clip, center, size).fit(footage::Fit::Fill),
        footage::media(id, seq(path), 0, scene.duration_nanos()),
    )?;
    let opacity = actor.channel(scene, "opacity");
    scene.set(&opacity, 0, 0.0);
    let time = actor.channel(scene, "time");
    scene.set(&time, 0, 0.5);
    Ok(actor)
}

fn cut(scene: &mut PlanBuilder, actor: &FootageActor, at: u64, frame: u32) {
    if at >= scene.duration_nanos() {
        return;
    }
    let time = actor.channel(scene, "time");
    scene.set(&time, at, frame as f32 + 0.5);
}

fn show(scene: &mut PlanBuilder, actor: &FootageActor, at: u64, visible: bool) {
    if at >= scene.duration_nanos() {
        return;
    }
    let opacity = actor.channel(scene, "opacity");
    scene.set(&opacity, at, if visible { 1.0 } else { 0.0 });
}

/// Where each part plays: `(start, end)` on the plan clock.
fn schedule(parts: &[Part], lines: &BTreeMap<&str, Vec<Audio>>, lead: u64) -> Vec<(u64, u64)> {
    let mut cursor = lead;
    parts
        .iter()
        .map(|part| {
            let start = (cursor as i64 + part.gap * MILLISECOND as i64).max(0) as u64;
            let length = lines[part.id].iter().map(Audio::duration).max().unwrap();
            cursor = start + length;
            (start, start + length)
        })
        .collect()
}

/// Words of `audio` placed at `start`, as `(text, start, end)` nanoseconds.
fn words(audio: &Audio, start: u64) -> Vec<(String, u64, u64)> {
    audio
        .transcript()
        .words()
        .iter()
        .map(|word| {
            (
                word.word.clone(),
                start + (word.start * 1e9) as u64,
                start + (word.end * 1e9) as u64,
            )
        })
        .collect()
}

fn council_film(
    id: &str,
    parts: &[Part],
    lines: &BTreeMap<&str, Vec<Audio>>,
    sounds: &Sounds,
    frames: &mut Frames,
    hook: bool,
) -> Result<ScenePlan> {
    let spans = schedule(parts, lines, millis(if hook { 600 } else { 500 }));
    for (part, (start, end)) in parts.iter().zip(&spans) {
        eprintln!(
            "  {id} {:>18} {:7.2}s – {:7.2}s",
            part.id,
            *start as f64 / 1e9,
            *end as f64 / 1e9
        );
    }
    let duration = spans.last().unwrap().1 + seconds(1.6);
    let mut scene = PlanBuilder::new(id, duration);
    let sc = &mut scene;
    // The screen boots after the hook line, or right away in the closer.
    let boot = if hook {
        spans[0].1 + millis(350)
    } else {
        millis(80)
    };

    let chrome = sequence(
        sc,
        &format!("{id}-chrome"),
        "chrome-%d.png",
        [960.0, 540.0],
        [1920.0, 1080.0],
        1920,
    )?;
    show(sc, &chrome, boot, true);
    let sheet = SpriteSheet::new(EXPRESSIONS)?;
    let mut bezels = Vec::new();
    let mut sprites = Vec::new();
    for (index, (slug, _)) in ADVISORS.iter().enumerate() {
        let center = panel_center(index);
        let bezel = sequence(
            sc,
            &format!("{id}-bezel-{slug}"),
            &format!("bezel-{slug}-%d.png"),
            center,
            BEZEL,
            468,
        )?;
        show(sc, &bezel, boot, true);
        bezels.push(bezel);
        let footage = sequence(
            sc,
            &format!("{id}-sprite-{slug}"),
            &format!("sprites/{slug}/%02d.png"),
            center,
            SPRITE,
            640,
        )?;
        let focus = footage.channel(sc, "focus-size");
        sc.set(&focus, 0, 0.96);
        sprites.push(Sprite::new(footage, sheet.clone(), SPRITE_FPS)?);
    }
    let agenda = sequence(
        sc,
        &format!("{id}-agenda"),
        "agenda-%d.png",
        AGENDA.0,
        AGENDA.1,
        384,
    )?;
    show(sc, &agenda, boot, true);
    let speech = sequence(
        sc,
        &format!("{id}-speech"),
        "speech/%04d.png",
        SPEECH.0,
        SPEECH.1,
        1840,
    )?;
    show(sc, &speech, boot, true);
    let banner = sequence(
        sc,
        &format!("{id}-banner"),
        "banners/%02d.png",
        [960.0, 540.0],
        BANNER,
        560,
    )?;
    let grain = FootageActor::declare(
        sc,
        format!("{id}-grain"),
        &FootagePlan::new(
            Clip::new(format!("{id}-grain"))
                .decoded_at(10)
                .looping()
                .resolution(960),
            [960.0, 540.0],
            [1920.0, 1080.0],
        )
        .fit(footage::Fit::Fill),
        footage::media(format!("{id}-grain"), seq("grain/%d.png"), 0, duration),
    )?;
    show(sc, &grain, 0, false);
    show(sc, &grain, boot, true);

    // Boot: the drive spins up and each well flickers to static, then a face.
    sounds.boot.play(sc, boot.saturating_sub(millis(600)), -8.0);
    let mut revealed = Vec::new();
    for (index, sprite) in sprites.iter().enumerate() {
        let at = boot + TICK * (2 + index as u64 * 2);
        show(sc, sprite.footage(), at, true);
        for flicker in 0..3u64 {
            sprite.show(
                sc,
                at + flicker * TICK,
                sheet.len() + (index as u32 + flicker as u32) % STATIC_FRAMES,
            );
        }
        revealed.push(at + 3 * TICK);
        sfx::TICK.play(sc, format!("{id}-flicker-{index}"), at, -22.0);
    }
    let awake = *revealed.last().unwrap();
    if hook {
        sounds.fanfare.play(sc, awake, -10.0);
    }
    let mut loop_at = awake;
    while loop_at + millis(200) < duration {
        let length = sounds.bed.duration().min(duration - loop_at);
        sc.media(MediaPlan {
            id: format!("{id}-bed@{loop_at}"),
            path: sounds.bed.path().to_owned(),
            kind: MediaKindPlan::Audio,
            role: MediaRolePlan::Layer,
            source_start_nanos: 0,
            source_end_nanos: length,
            timeline_start_nanos: loop_at,
            timeline_end_nanos: loop_at + length,
            gain_db: -27.0,
        });
        loop_at += length;
    }

    // Per-advisor performance: expressions, mouths, and when they speak.
    let mut expressions: Vec<Vec<(u64, u32)>> = vec![Vec::new(); ADVISORS.len()];
    let mut mouths: Vec<Vec<(u64, Mouth)>> = vec![Vec::new(); ADVISORS.len()];
    let mut speaking: Vec<Vec<(u64, u64)>> = vec![Vec::new(); ADVISORS.len()];
    let mut agenda_act = None;
    for (index, (part, &(start, end))) in parts.iter().zip(&spans).enumerate() {
        let audios = &lines[part.id];
        for audio in audios {
            audio.play(sc, start, if part.speakers.len() > 1 { -8.0 } else { 0.0 });
        }
        if agenda_act != Some(part.act) {
            cut(
                sc,
                &agenda,
                start.max(boot).saturating_sub(millis(300)),
                part.act as u32,
            );
            agenda_act = Some(part.act);
        }
        for (speaker, audio) in part.speakers.iter().zip(audios) {
            let Some(advisor) = advisor_index(speaker) else {
                continue;
            };
            let expression = sheet.expression(part.expression)?;
            expressions[advisor].push((start.saturating_sub(TICK), expression));
            let spoken = words(audio, start);
            if let Some((word, face)) = part.turn {
                let normalize = |text: &str| {
                    text.chars()
                        .filter(|c| c.is_alphanumeric())
                        .collect::<String>()
                        .to_lowercase()
                };
                let (_, at, _) = spoken
                    .iter()
                    .find(|(text, _, _)| normalize(text) == word)
                    .with_context(|| format!("{} never says '{word}'", part.id))?;
                expressions[advisor].push((*at, sheet.expression(face)?));
            }
            mouths[advisor].extend(lipsync::visemes(
                spoken
                    .iter()
                    .map(|(word, from, to)| (word.as_str(), *from, *to)),
                TICK,
            ));
            speaking[advisor].push((start, end));
        }
        // The interrupted advisor bristles while the shout lands.
        if part.banner.is_some()
            && index > 0
            && let Some(previous) = parts[index - 1]
                .speakers
                .first()
                .and_then(|s| advisor_index(s))
            && !part.speakers.contains(&ADVISORS[previous].0)
        {
            let reaction = if part.speakers == ["contrarian"] {
                "annoyed"
            } else {
                "excited"
            };
            expressions[previous].push((start + millis(250), sheet.expression(reaction)?));
            expressions[previous].push((end + millis(400), sheet.expression("neutral")?));
        }
        if let Some(text) = part.banner {
            let frame = frames.banner(text);
            let advisor = advisor_index(part.speakers[0]).unwrap();
            let [x, y] = panel_center(advisor);
            for (property, value) in [("x", x - 960.0), ("y", y - 540.0 + SPRITE[1] * 0.5 - 52.0)] {
                let channel = banner.channel(sc, property);
                sc.set(&channel, start, value);
            }
            cut(sc, &banner, start, frame);
            show(sc, &banner, start, true);
            let scale = banner.channel(sc, "scale");
            sc.set(&scale, start, 1.35);
            sc.set(&scale, start + TICK, 0.92);
            sc.set(&scale, start + 2 * TICK, 1.0);
            show(sc, &banner, (start + seconds(2.4)).min(end), false);
            sounds.sting.play(sc, start, -12.0);
        }

        // The speech box pages through the line, highlighting each word.
        if part.speakers == ["narrator"] && start < boot {
            continue;
        }
        let spoken = words(&audios[0], start);
        let label = label_of(part.speakers);
        let mut page: Vec<usize> = Vec::new();
        let mut pages = Vec::new();
        let mut length = 0;
        for (index, (word, _, _)) in spoken.iter().enumerate() {
            if length + word.len() > 150 && !page.is_empty() {
                pages.push(std::mem::take(&mut page));
                length = 0;
            }
            length += word.len() + 1;
            page.push(index);
        }
        pages.extend((!page.is_empty()).then_some(page));
        for page in pages {
            let texts: Vec<String> = page.iter().map(|&i| spoken[i].0.clone()).collect();
            for (offset, &word) in page.iter().enumerate() {
                let frame = frames.page(label, &texts, Some(offset));
                cut(sc, &speech, spoken[word].1, frame);
            }
        }
        let next = spans.get(index + 1).map_or(duration, |span| span.0);
        if next > end + seconds(1.2) {
            cut(sc, &speech, end + millis(600), 0);
        }
    }

    for (advisor, sprite) in sprites.iter().enumerate() {
        let reveal = revealed[advisor];
        let mut faces = vec![(0, 0)];
        let mut sorted = std::mem::take(&mut expressions[advisor]);
        sorted.sort_by_key(|(at, _)| *at);
        faces.extend(sorted.into_iter().map(|(at, e)| (at.max(reveal), e)));
        // Return to neutral a beat after each line.
        for &(_, end) in &speaking[advisor] {
            faces.push((end + millis(1300), 0));
        }
        faces.sort_by_key(|(at, _)| *at);
        let blinks = lipsync::blinks(reveal + millis(400), duration, advisor as u32 * 7 + 3);
        let cuts: Vec<(u64, u32)> = lipsync::frames(&sheet, &faces, &mouths[advisor], &blinks)
            .into_iter()
            .filter(|(at, _)| *at >= reveal && *at < duration)
            .collect();
        sprite.show(sc, reveal, sheet.frame(0, Mouth::Rest));
        sprite.perform(sc, &cuts);

        // A slight idle bob inside the well, choppier and larger while speaking.
        let focus = sprite.footage().channel(sc, "focus-y");
        let mut at = reveal / TICK * TICK;
        while at < duration {
            let talking = speaking[advisor]
                .iter()
                .any(|&(from, to)| from <= at && at < to);
            let amp = if talking { 0.011 } else { 0.004 };
            let t = at as f32 / SECOND as f32;
            sc.set(
                &focus,
                at,
                0.5 + amp * smooth_noise(t * if talking { 2.2 } else { 0.7 }, advisor as u32 + 11),
            );
            at += TICK * 2;
        }
        // The well lights gold while its advisor speaks.
        for &(from, to) in &speaking[advisor] {
            cut(sc, &bezels[advisor], from, 1);
            cut(sc, &bezels[advisor], to + millis(150), 0);
        }
    }
    // Fade the whole screen to black at the end of the closer.
    if !hook {
        let fade = duration - millis(700);
        for actor in [&chrome, &agenda, &speech, &grain]
            .into_iter()
            .chain(bezels.iter())
            .chain(sprites.iter().map(Sprite::footage))
        {
            show(sc, actor, fade, false);
        }
    }
    Ok(scene.finish()?)
}

/// The pipeline on the Stage: schedule, advisor sub-agents, repo and issues,
/// pitches, voices, council.
fn how_it_works(how: &Audio, voiced: &Audio) -> Result<ScenePlan> {
    let lead = millis(700);
    let duration = lead + how.duration() + millis(500) + voiced.duration() + seconds(1.4);
    let mut scene = PlanBuilder::new("how-it-works", duration);
    let a = how.place(&mut scene, lead);
    let b = voiced.place(&mut scene, lead + how.duration() + millis(500));
    let mut stage = StageActor::declare(&mut scene, "stage", &stage_plan())?;
    let s = &mut stage;
    let sc = &mut scene;
    for label in ["title", "no-video"] {
        s.channel(sc, &format!("{label}.opacity"), 0.0);
    }
    s.type_in(sc, "title", millis(200), 40.0);

    let wake = a.at("schedule");
    let at = s.settle_in(sc, "schedule", wake);
    s.clock(sc, "schedule.spinner", wake);
    sfx::TICK.play(sc, "schedule-in", wake, -18.0);

    let agents = a.at("sub-agent");
    for (index, (slug, _)) in ADVISORS.iter().enumerate() {
        let face = format!("face-{slug}");
        sc.media(footage::still(
            &face,
            seq(&format!("sprites/{slug}/00.png")),
            0,
            duration,
        ));
        s.fade_in(sc, &face, agents + millis(120) * index as u64, 1.0, 0.3);
        s.channel(sc, &format!("{face}.y"), 24.0);
        s.to(
            sc,
            &format!("{face}.y"),
            agents + millis(120) * index as u64,
            0.0,
            0.45,
        );
    }
    let contact = s.connect(sc, "wake", agents - millis(150), 0.5);
    let sent = s.send(sc, "wake-up", contact.max(at), 0.55);
    sfx::SEND.play(sc, "wake-send", contact.max(at), -12.0);
    s.settle_in(sc, "advisors", agents);
    s.land(sc, "advisors", sent);

    let research = a.at("researches");
    s.settle_in(sc, "repo", research);
    s.connect(sc, "research", research + millis(200), 0.5);
    s.send(sc, "query", research + millis(600), 0.5);
    let back = s.send(sc, "evidence", research + millis(1500), 0.5);
    s.land(sc, "advisors", back);
    s.swap_status(sc, "advisors", research + millis(300), [0, 1], 0.3);
    s.swap_status(sc, "repo", research + millis(900), [0, 1], 0.3);

    let pitch = a.at("one pitch");
    s.settle_in(sc, "pitches", pitch);
    s.connect(sc, "write", pitch - millis(100), 0.45);
    let landed = s.send(sc, "pitch", pitch + millis(200), 0.5);
    s.land(sc, "pitches", landed);
    s.swap_status(sc, "advisors", pitch, [1, 2], 0.3);
    sfx::MARK.play(sc, "pitch-mark", landed, -14.0);

    let voice = b.at("designed voice");
    s.settle_in(sc, "voices", voice);
    s.connect(sc, "speak", voice - millis(100), 0.45);
    let spoken = s.send(sc, "line", voice + millis(200), 0.5);
    s.land(sc, "voices", spoken);
    let stamps = b.at("timestamps");
    s.swap_status(sc, "voices", stamps, [0, 1], 0.3);
    let mouth = b.at("mouth shape");
    s.settle_in(sc, "council", mouth);
    s.connect(sc, "drive", mouth - millis(100), 0.45);
    let synced = s.send(sc, "visemes", mouth + millis(200), 0.5);
    s.land(sc, "council", synced);
    s.swap_status(sc, "council", b.at("portraits"), [0, 1], 0.3);
    let no = b.at("No AI video");
    s.type_in(sc, "no-video", no, 34.0);
    s.hit(sc, "council.flash", b.at("swapped"), 0.6, 0.0);
    s.swap_status(sc, "council", b.at("ninety-six"), [1, 2], 0.3);
    sfx::CONFIRM.play(sc, "ninety-six", b.at("ninety-six"), -14.0);
    Ok(scene.finish()?)
}

const PORTRAIT: [f32; 2] = [128.0, 96.0];

fn portrait_at(index: usize) -> [f32; 3] {
    [760.0 + (index as f32 - 2.5) * 140.0, 700.0, 0.0]
}

fn stage_plan() -> StagePlan {
    const Y: f32 = 520.0;
    let mut elements = vec![
        StageElement::label(
            "title",
            [960.0, 130.0, 0.0],
            34.0,
            &[("how the council convenes", Tone::Muted)],
        ),
        StageElement::card("schedule", [250.0, Y, 0.0], [340.0, 150.0], "every morning")
            .statuses(&[("06:00 · schedule fires", Tone::Accent)])
            .tone(Tone::Accent),
        StageElement::card(
            "advisors",
            [760.0, Y, 0.0],
            [400.0, 150.0],
            "6 advisor sub-agents",
        )
        .statuses(&[
            ("one job each", Tone::Plain),
            ("researching…", Tone::Warning),
            ("one pitch each", Tone::Success),
        ])
        .tone(Tone::Request),
        StageElement::card(
            "repo",
            [760.0, 270.0, -10.0],
            [420.0, 130.0],
            "anomalyco/opencode",
        )
        .statuses(&[
            ("origin/v2 · code", Tone::Plain),
            ("+ open issues", Tone::Warning),
        ])
        .tone(Tone::Plain),
        StageElement::card("pitches", [1290.0, Y, 0.0], [340.0, 150.0], "pitches.json")
            .statuses(&[("idea + evidence", Tone::Success)])
            .tone(Tone::Success),
        StageElement::card(
            "voices",
            [1290.0, 880.0, 0.0],
            [380.0, 150.0],
            "designed voices",
        )
        .statuses(&[
            ("ElevenLabs v4", Tone::Plain),
            ("+ timestamps", Tone::Accent),
        ])
        .tone(Tone::Warning),
        StageElement::card(
            "council",
            [1690.0, 520.0, 0.0],
            [380.0, 150.0],
            "council screen",
        )
        .statuses(&[
            ("lip-synced sprites", Tone::Plain),
            ("portraits × mouth shapes", Tone::Accent),
            ("1996 mode", Tone::Success),
        ])
        .tone(Tone::Accent),
        StageElement::beam("wake", "schedule", "advisors").tone(Tone::Accent),
        StageElement::beam("research", "advisors", "repo")
            .bend(-30.0)
            .tone(Tone::Warning),
        StageElement::beam("write", "advisors", "pitches").tone(Tone::Success),
        StageElement::beam("speak", "pitches", "voices").tone(Tone::Warning),
        StageElement::beam("drive", "voices", "council")
            .bend(40.0)
            .tone(Tone::Accent),
        StageElement::packet("wake-up", "wake")
            .labeled("good morning")
            .tone(Tone::Accent),
        StageElement::packet("query", "research")
            .labeled("read code & issues")
            .tone(Tone::Warning),
        StageElement::packet("evidence", "research")
            .reversed()
            .labeled("file:line · #issue")
            .tone(Tone::Warning),
        StageElement::packet("pitch", "write")
            .labeled("one pitch")
            .tone(Tone::Success),
        StageElement::packet("line", "speak")
            .labeled("in character")
            .tone(Tone::Warning),
        StageElement::packet("visemes", "drive")
            .labeled("mouth cues")
            .tone(Tone::Accent),
        StageElement::label(
            "no-video",
            [1690.0, 360.0, 0.0],
            32.0,
            &[
                ("no AI video · ", Tone::Muted),
                ("just sprites", Tone::Accent),
            ],
        ),
    ];
    for (index, (slug, _)) in ADVISORS.iter().enumerate() {
        elements.push(StageElement::Footage {
            id: format!("face-{slug}"),
            at: portrait_at(index),
            size: PORTRAIT,
            clip: Clip::new(format!("face-{slug}")).frozen(0.0),
            fit: footage::Fit::Cover,
            mask: footage::Mask::default(),
            framed: true,
            tint: Tone::Accent,
        });
    }
    StagePlan {
        post: StagePost::RESTRAINED,
        elements,
    }
}

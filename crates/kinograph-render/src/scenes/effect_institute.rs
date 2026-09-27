use std::{
    cell::OnceCell,
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};

use kinograph::{
    code::{CodeLine, PlacedLine, StyledSpan, SyntaxStyle},
    composition::{Asset, Clip, Composition, Duration, Time, TimeRange},
    dsl::{
        AnnotationEffect, AnnotationFrame, CompiledScene, Scalar, Scene, TargetGeometry, TaskFrame,
        TaskId, TaskState,
    },
    motion::MotionState,
    state::{StateTrack, TimedState},
    timeline::{Animation, PropertyId, SpringProfile, TimedEvent, Timeline},
};

use crate::render::{
    BrightTextFrame, EditorFrame, HeadlessRenderer, InlineRevealFrame, PointerFrame, RenderSpec,
    SquiggleFrame, TaskLinkFrame, TaskSceneFrame, TextRangeBounds, TokenHighlight,
};

use super::{FONT_PATH, HEIGHT, WIDTH, WORKSPACE_ROOT, encode_video_with_samples};

const LINE_HEIGHT: f32 = 44.0;
const OPENER_DURATION: f64 = 3.0;
const GROUP_DURATION: f64 = 2.5;
const GAP_DURATION: f64 = 0.5;
const OUTRO_DURATION: f64 = 0.5;

pub(crate) async fn render(chapter_id: &str, output: &Path) -> Result<()> {
    let assets = Path::new(WORKSPACE_ROOT)
        .join("assets/effect-institute")
        .join(chapter_id);
    let chapter = PublishedChapter::load(chapter_id, &assets)?;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: chapter.title.clone(),
    })
    .await?;

    encode_video_with_samples(
        &mut renderer,
        output,
        &chapter.scene,
        2,
        2,
        &[],
        |renderer, time| chapter.render_sample(renderer, time),
    )
}

pub(crate) async fn render_section(
    chapter_id: &str,
    section_id: &str,
    output: &Path,
) -> Result<()> {
    let assets = Path::new(WORKSPACE_ROOT)
        .join("assets/effect-institute")
        .join(chapter_id);
    let chapter = PublishedChapter::load(chapter_id, &assets)?;
    let section = chapter
        .sections
        .iter()
        .find(|section| section.id == section_id)
        .with_context(|| format!("chapter '{chapter_id}' has no section '{section_id}'"))?;
    let scene = Scene::new(Vec::<(PropertyId, Scalar)>::new(), section.composition())
        .compile(&HashMap::new())?;
    let mut renderer = HeadlessRenderer::new(RenderSpec {
        width: WIDTH,
        height: HEIGHT,
        font_path: PathBuf::from(FONT_PATH),
        file_name: format!("{chapter_id}/{section_id}.ts"),
    })
    .await?;
    encode_video_with_samples(
        &mut renderer,
        output,
        &scene,
        2,
        2,
        &[],
        |renderer, time| section.render(renderer, time),
    )
}

struct PublishedChapter {
    title: String,
    scene: CompiledScene,
    sections: Vec<PublishedSection>,
    segments: Vec<ChapterSegment>,
}

enum ChapterSegment {
    Title {
        range: TimeRange,
        title: String,
        subtitle: Option<String>,
    },
    Gap {
        range: TimeRange,
    },
    Section {
        range: TimeRange,
        index: usize,
    },
}

impl ChapterSegment {
    fn range(&self) -> TimeRange {
        match self {
            Self::Title { range, .. } | Self::Gap { range } | Self::Section { range, .. } => *range,
        }
    }
}

impl PublishedChapter {
    fn load(chapter_id: &str, directory: &Path) -> Result<Self> {
        let manifest: PublishedManifest = serde_json::from_slice(
            &fs::read(directory.join("manifest.json")).context("read chapter manifest")?,
        )
        .context("parse chapter manifest")?;
        if manifest.id != chapter_id {
            bail!(
                "chapter manifest '{}' does not match requested '{chapter_id}'",
                manifest.id
            );
        }

        let excluded_groups = HashSet::from(["welcome"]);
        let section_files = manifest
            .sections
            .iter()
            .map(|section| (section.id.as_str(), section.file.as_str()))
            .collect::<HashMap<_, _>>();
        let mut sections = Vec::new();
        let mut section_groups = Vec::new();
        for group in &manifest.groups {
            if excluded_groups.contains(group.id.as_str()) {
                continue;
            }
            for id in &group.section_ids {
                let file = section_files
                    .get(id.as_str())
                    .with_context(|| format!("manifest has no file for section '{id}'"))?;
                sections.push(PublishedSection::load(chapter_id, directory, file)?);
                section_groups.push(group.title.clone());
            }
        }
        if sections.is_empty() {
            bail!("chapter '{chapter_id}' has no selected sections");
        }

        let mut segments = Vec::new();
        let mut cursor = Time::ZERO;
        let opener = Duration::seconds(OPENER_DURATION);
        segments.push(ChapterSegment::Title {
            range: TimeRange::new(cursor, cursor.after(opener)),
            title: manifest.title.clone(),
            subtitle: section_groups.first().cloned(),
        });
        cursor = cursor.after(opener);

        for index in 0..sections.len() {
            if index > 0 {
                let group_changed = section_groups[index] != section_groups[index - 1];
                let gap = if group_changed {
                    Duration::seconds(GROUP_DURATION)
                } else {
                    Duration::seconds(GAP_DURATION)
                };
                let range = TimeRange::new(cursor, cursor.after(gap));
                if group_changed {
                    segments.push(ChapterSegment::Title {
                        range,
                        title: section_groups[index].clone(),
                        subtitle: None,
                    });
                } else {
                    segments.push(ChapterSegment::Gap { range });
                }
                cursor = range.end();
            }

            let section = &sections[index];
            let range = TimeRange::new(cursor, cursor.after(section.duration));
            segments.push(ChapterSegment::Section { range, index });
            cursor = range.end();
        }
        let outro = Duration::seconds(OUTRO_DURATION);
        segments.push(ChapterSegment::Gap {
            range: TimeRange::new(cursor, cursor.after(outro)),
        });

        let composition = segments.iter().map(|segment| match segment {
            ChapterSegment::Title { range, .. } | ChapterSegment::Gap { range } => {
                Composition::hold(range.duration())
            }
            ChapterSegment::Section { index, range } => {
                let section = &sections[*index];
                assert_eq!(section.duration, range.duration());
                Composition::named(section.id.clone(), section.composition())
            }
        });

        let scene = Scene::new(
            Vec::<(PropertyId, Scalar)>::new(),
            Composition::sequence(composition),
        )
        .compile(&HashMap::new())?;
        Ok(Self {
            title: manifest.title,
            scene,
            sections,
            segments,
        })
    }

    fn render_sample(&self, renderer: &mut HeadlessRenderer, time: f32) -> Result<Vec<u8>> {
        let time = Time::seconds(f64::from(time.max(0.0)));
        let segment = self
            .segments
            .iter()
            .find(|segment| {
                let range = segment.range();
                time >= range.start() && time < range.end()
            })
            .or_else(|| self.segments.last())
            .expect("chapter has at least one segment");
        match segment {
            ChapterSegment::Title {
                range,
                title,
                subtitle,
            } => {
                let local = TimeRange::new(range.start(), time).duration().as_seconds() as f32;
                let duration = range.duration().as_seconds() as f32;
                let opacity = title_opacity(local, duration);
                Ok(renderer.render_title_card(title, subtitle.as_deref(), opacity))
            }
            ChapterSegment::Gap { .. } => Ok(renderer.render_title_card("", None, 0.0)),
            ChapterSegment::Section { range, index } => {
                let local = TimeRange::new(range.start(), time).duration().as_seconds() as f32;
                self.sections[*index].render(renderer, local)
            }
        }
    }
}

fn title_opacity(local: f32, duration: f32) -> f32 {
    let fade = 0.4_f32.min(duration * 0.3);
    if local < fade {
        smoothstep((local / fade).clamp(0.0, 1.0))
    } else if local > duration - fade {
        smoothstep(((duration - local) / fade).clamp(0.0, 1.0))
    } else {
        1.0
    }
}

struct PublishedSection {
    chapter_id: String,
    id: String,
    duration: Duration,
    narration: Clip,
    code: Option<PublishedCode>,
    component: Option<PublishedComponent>,
    sound_layers: Vec<PublishedSoundLayer>,
}

struct PublishedSoundLayer {
    at: Duration,
    clip: Clip,
}

impl PublishedSection {
    fn load(chapter_id: &str, directory: &Path, file: &str) -> Result<Self> {
        let raw: PublishedSectionJson = serde_json::from_slice(
            &fs::read(directory.join(file))
                .with_context(|| format!("read published section '{file}'"))?,
        )
        .with_context(|| format!("parse published section '{file}'"))?;
        let audio_file = raw
            .audio_file
            .as_ref()
            .with_context(|| format!("section '{}' has no audio file", raw.id))?;
        let audio_path = directory.join(audio_file);
        let duration = probe_audio_duration(&audio_path)?;
        let narration =
            Asset::audio(format!("{chapter_id}.{}.narration", raw.id), audio_path).clip(
                TimeRange::new(Time::ZERO, Time::seconds(duration.as_seconds())),
            );
        let mut sound_cues = Vec::new();
        if let (Some(slideshow), Some(step_times)) =
            (raw.slideshow_data.as_ref(), raw.step_times.as_ref())
        {
            for (frame, at) in slideshow.frames.iter().zip(step_times) {
                if let Some(sound) = &frame.sound {
                    sound_cues.push((*at, sound.clone()));
                }
            }
        }
        if let (Some(flow), Some(step_times)) = (
            raw.component_flow.as_ref(),
            raw.component_step_times.as_ref(),
        ) {
            for (snapshot, at) in flow.snapshots.iter().zip(step_times) {
                if let Some(sound) = snapshot.get("_sound").and_then(PublishedValue::as_str) {
                    sound_cues.push((*at, sound.to_owned()));
                }
            }
        }
        let sound_layers = build_sound_layers(chapter_id, &raw.id, duration, &sound_cues)?;
        let component = raw
            .component_flow
            .map(|flow| {
                let step_times = raw.component_step_times.unwrap_or_default();
                if flow.snapshots.len() != step_times.len() || flow.snapshots.is_empty() {
                    bail!(
                        "section '{}' has mismatched component snapshots and step times",
                        raw.id
                    );
                }
                PublishedComponent::compile(flow, step_times, duration)
            })
            .transpose()?;
        let code = match raw.slideshow_data {
            Some(slideshow) => Some(PublishedCode::compile(
                chapter_id,
                &raw.id,
                slideshow,
                raw.step_times
                    .with_context(|| format!("section '{}' has no step times", raw.id))?,
            )?),
            None if component
                .as_ref()
                .is_some_and(|value| value.flow.name == "EffectTypeDisplay") =>
            {
                let (slideshow, times) = type_display_slideshow(component.as_ref().unwrap());
                Some(PublishedCode::compile(
                    chapter_id, &raw.id, slideshow, times,
                )?)
            }
            None => None,
        };
        Ok(Self {
            chapter_id: chapter_id.to_owned(),
            id: raw.id,
            duration,
            narration,
            code,
            component,
            sound_layers,
        })
    }

    fn composition(&self) -> Composition {
        let mut layers = vec![
            Composition::script(self.narration.clone()),
            Composition::hold(self.duration),
        ];
        layers.extend(
            self.sound_layers
                .iter()
                .map(|sound| Composition::delay(sound.at, Composition::layer(sound.clip.clone()))),
        );
        Composition::parallel(layers)
    }

    fn render(&self, renderer: &mut HeadlessRenderer, time: f32) -> Result<Vec<u8>> {
        renderer.set_file_name(&format!("{}/{}.ts", self.chapter_id, self.id));
        let mut pixels = if self.id == "types-capture-runtime" {
            self.code
                .as_ref()
                .context("type display has no prepared code")?
                .render_type_display(renderer, time)?
        } else if let Some(code) = &self.code {
            code.render(renderer, time)?
        } else {
            renderer.render_title_card(&self.id.replace('-', " "), None, 1.0)
        };
        if let Some(component) = &self.component {
            component.composite(renderer, &mut pixels, time)?;
        }
        Ok(pixels)
    }
}

fn build_sound_layers(
    chapter_id: &str,
    section_id: &str,
    section_duration: Duration,
    cues: &[(f64, String)],
) -> Result<Vec<PublishedSoundLayer>> {
    let root = Path::new(WORKSPACE_ROOT);
    cues.iter()
        .enumerate()
        .filter_map(|(index, (at, sound))| {
            let at = Duration::seconds((*at).max(0.0));
            (at < section_duration).then_some((index, at, sound))
        })
        .map(|(index, at, sound)| {
            let relative = match sound.as_str() {
                "sad" => "assets/promises-only-happy-path/sad.wav",
                "fail" => "assets/visual-effects/task-failure.wav",
                "success" | "succeed" => "assets/visual-effects/task-success.wav",
                "ping1" => "assets/opencode-hot-reload/confirm.wav",
                "whoosh" => "assets/opencode-hot-reload/launch.wav",
                "alarm" => "assets/opencode-hot-reload/impact.wav",
                "spooky" => "assets/visual-effects/task-death.wav",
                _ => return Err(anyhow::anyhow!("unknown published sound cue '{sound}'")),
            };
            let path = root.join(relative);
            let asset_duration = probe_audio_duration(&path)?;
            let remaining = section_duration.as_seconds() - at.as_seconds();
            let clip_duration = asset_duration.as_seconds().min(remaining);
            let clip = Asset::audio(
                format!("{chapter_id}.{section_id}.sound.{index}.{sound}"),
                path,
            )
            .clip(TimeRange::new(Time::ZERO, Time::seconds(clip_duration)));
            Ok(PublishedSoundLayer { at, clip })
        })
        .collect()
}

fn probe_audio_duration(path: &Path) -> Result<Duration> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .with_context(|| format!("probe audio duration for {}", path.display()))?;
    if !output.status.success() {
        bail!("ffprobe failed for {}", path.display());
    }
    let seconds = String::from_utf8(output.stdout)?
        .trim()
        .parse::<f64>()
        .with_context(|| format!("parse audio duration for {}", path.display()))?;
    Ok(Duration::seconds(seconds))
}

struct PublishedCode {
    lines: Vec<PreparedLine>,
    timeline: Timeline,
    frames: Vec<PublishedFrame>,
    step_times: Vec<f32>,
    overlays: OverlayAnimation,
}

struct PreparedLine {
    source_id: String,
    occurrence: usize,
    line: CodeLine,
    y: PropertyId,
    opacity: PropertyId,
    blur: PropertyId,
    variants: Vec<PreparedVariant>,
}

struct PreparedVariant {
    part_id: String,
    version: Option<String>,
    start_span: usize,
    end_span: usize,
    progress: PropertyId,
}

struct OverlayAnimation {
    cursor_x_columns: AnalyticTrack,
    cursor_y: AnalyticTrack,
    cursor_rest_x: AnalyticTrack,
    cursor_rest_y: AnalyticTrack,
    cursor_opacity: AnalyticTrack,
    cursor_scale: AnalyticTrack,
    cursor_blur: AnalyticTrack,
    highlight_x_columns: AnalyticTrack,
    highlight_y: AnalyticTrack,
    highlight_width_columns: AnalyticTrack,
    highlight_opacity: AnalyticTrack,
    focus_y: AnalyticTrack,
    focus_height: AnalyticTrack,
    focus_intensity: AnalyticTrack,
    scroll_y: AnalyticTrack,
}

struct AnalyticTrack {
    property: PropertyId,
    initial: f32,
    events: Vec<TimedEvent>,
    profile: SpringProfile,
    timeline: OnceCell<Timeline>,
}

impl AnalyticTrack {
    fn new(id: &str, initial: f32, visual_duration: f32, bounce: f32) -> Self {
        Self {
            property: PropertyId::new(format!("published-overlay.{id}")),
            initial,
            events: Vec::new(),
            profile: SpringProfile::from_visual_duration(visual_duration, bounce, 0.0001, 0.0001),
            timeline: OnceCell::new(),
        }
    }

    fn set(&mut self, time: f32, target: f32) {
        self.events.push(TimedEvent::set(
            f64::from(time),
            self.property.clone(),
            target,
        ));
    }

    fn spring_to(&mut self, time: f32, target: f32) {
        self.events.push(TimedEvent::spring(
            f64::from(time),
            self.property.clone(),
            target,
            self.profile,
        ));
    }

    fn sample(&self, time: f32) -> MotionState {
        self.timeline
            .get_or_init(|| {
                let duration = self.events.iter().map(TimedEvent::at).fold(0.0, f64::max)
                    + f64::from(self.profile.advance_time());
                Timeline::compile_events(
                    [(self.property.clone(), self.initial)],
                    self.events.clone(),
                    duration,
                )
                .expect("published overlay events were validated during import")
            })
            .sample(&self.property, time)
            .expect("published overlay track has an initial value")
    }
}

impl OverlayAnimation {
    fn compile(frames: &[PublishedFrame], step_times: &[f32], initial_frame: usize) -> Self {
        let first_cursor = frames
            .iter()
            .find_map(cursor_target)
            .unwrap_or(CursorTarget {
                x_columns: 0.0,
                y: 0.0,
            });
        let initial_cursor = cursor_target(&frames[initial_frame]);
        let cursor_origin = initial_cursor.unwrap_or(first_cursor);
        let cursor_visible = initial_cursor.is_some();
        let initial_scroll = frame_scroll_target(&frames[initial_frame]);
        let mut animation = Self {
            cursor_x_columns: AnalyticTrack::new(
                "cursor.x-columns",
                cursor_origin.x_columns,
                0.5,
                0.35,
            ),
            cursor_y: AnalyticTrack::new("cursor.y", cursor_origin.y, 0.5, 0.35),
            cursor_rest_x: AnalyticTrack::new(
                "cursor.rest-x",
                if cursor_visible { 0.0 } else { 40.0 },
                0.5,
                0.35,
            ),
            cursor_rest_y: AnalyticTrack::new(
                "cursor.rest-y",
                if cursor_visible { 0.0 } else { 30.0 },
                0.5,
                0.35,
            ),
            cursor_opacity: AnalyticTrack::new(
                "cursor.opacity",
                f32::from(cursor_visible),
                0.5,
                0.35,
            ),
            cursor_scale: AnalyticTrack::new(
                "cursor.scale",
                if cursor_visible { 1.0 } else { 0.7 },
                0.5,
                0.35,
            ),
            cursor_blur: AnalyticTrack::new(
                "cursor.blur",
                if cursor_visible { 0.0 } else { 4.0 },
                0.5,
                0.35,
            ),
            highlight_x_columns: AnalyticTrack::new("highlight.x-columns", 0.0, 0.6, 0.1),
            highlight_y: AnalyticTrack::new("highlight.y", 0.0, 0.6, 0.1),
            highlight_width_columns: AnalyticTrack::new("highlight.width-columns", 0.0, 0.6, 0.1),
            highlight_opacity: AnalyticTrack::new("highlight.opacity", 0.0, 0.6, 0.1),
            focus_y: AnalyticTrack::new("focus.y", 0.0, 0.4, 0.0),
            focus_height: AnalyticTrack::new("focus.height", LINE_HEIGHT, 0.4, 0.0),
            focus_intensity: AnalyticTrack::new("focus.intensity", 0.0, 0.4, 0.0),
            scroll_y: AnalyticTrack::new("scroll.y", initial_scroll, 0.45, 0.0),
        };

        let first_highlight = frames.iter().find_map(highlight_target);
        let mut previous_highlight = highlight_target(&frames[initial_frame]);
        let highlight_origin = previous_highlight
            .or(first_highlight)
            .unwrap_or(HighlightTarget {
                x_columns: 0.0,
                y: 0.0,
                width_columns: 0.0,
            });
        animation.highlight_x_columns.initial = if previous_highlight.is_some() {
            highlight_origin.x_columns
        } else {
            highlight_origin.x_columns + highlight_origin.width_columns * 0.5
        };
        animation.highlight_y.initial = highlight_origin.y;
        animation.highlight_width_columns.initial = if previous_highlight.is_some() {
            highlight_origin.width_columns
        } else {
            0.0
        };
        animation.highlight_opacity.initial = if previous_highlight.is_some() {
            0.8
        } else {
            0.0
        };

        let first_focus = frames.iter().find_map(focus_target);
        let mut previous_focus = focus_target(&frames[initial_frame]);
        let focus_origin = previous_focus.or(first_focus).unwrap_or(FocusTarget {
            y: 0.0,
            height: LINE_HEIGHT,
        });
        animation.focus_y.initial = focus_origin.y;
        animation.focus_height.initial = focus_origin.height;
        animation.focus_intensity.initial = f32::from(previous_focus.is_some());

        let mut previous_cursor = initial_cursor;
        let mut index = initial_frame + 1;
        while index < frames.len() {
            let time = step_times[index];
            let mut effective = index;
            while effective + 1 < frames.len() && step_times[effective + 1] == time {
                effective += 1;
            }
            let frame = &frames[effective];
            animation
                .scroll_y
                .spring_to(time, frame_scroll_target(frame));
            let current_cursor = cursor_target(frame);
            match (previous_cursor, current_cursor) {
                (Some(_), Some(target)) => {
                    animation.cursor_x_columns.spring_to(time, target.x_columns);
                    animation.cursor_y.spring_to(time, target.y);
                }
                (None, Some(target)) => {
                    animation.cursor_x_columns.set(time, target.x_columns);
                    animation.cursor_y.set(time, target.y);
                    animation.cursor_rest_x.set(time, 40.0);
                    animation.cursor_rest_y.set(time, 30.0);
                    animation.cursor_opacity.set(time, 0.0);
                    animation.cursor_scale.set(time, 0.7);
                    animation.cursor_blur.set(time, 4.0);
                    animation.cursor_rest_x.spring_to(time, 0.0);
                    animation.cursor_rest_y.spring_to(time, 0.0);
                    animation.cursor_opacity.spring_to(time, 1.0);
                    animation.cursor_scale.spring_to(time, 1.0);
                    animation.cursor_blur.spring_to(time, 0.0);
                }
                (Some(_), None) => {
                    animation.cursor_rest_x.spring_to(time, 40.0);
                    animation.cursor_rest_y.spring_to(time, 30.0);
                    animation.cursor_opacity.spring_to(time, 0.0);
                    animation.cursor_scale.spring_to(time, 1.15);
                    animation.cursor_blur.spring_to(time, 2.0);
                }
                (None, None) => {}
            }
            previous_cursor = current_cursor;

            let current_highlight = highlight_target(frame);
            match (previous_highlight, current_highlight) {
                (Some(_), Some(target)) => {
                    animation
                        .highlight_x_columns
                        .spring_to(time, target.x_columns);
                    animation.highlight_y.spring_to(time, target.y);
                    animation
                        .highlight_width_columns
                        .spring_to(time, target.width_columns);
                }
                (None, Some(target)) => {
                    animation
                        .highlight_x_columns
                        .set(time, target.x_columns + target.width_columns * 0.5);
                    animation.highlight_y.set(time, target.y);
                    animation.highlight_width_columns.set(time, 0.0);
                    animation.highlight_opacity.set(time, 0.0);
                    animation
                        .highlight_x_columns
                        .spring_to(time, target.x_columns);
                    animation
                        .highlight_width_columns
                        .spring_to(time, target.width_columns);
                    animation.highlight_opacity.spring_to(time, 0.8);
                }
                (Some(target), None) => {
                    animation
                        .highlight_x_columns
                        .spring_to(time, target.x_columns + target.width_columns * 0.5);
                    animation.highlight_width_columns.spring_to(time, 0.0);
                    animation.highlight_opacity.spring_to(time, 0.0);
                }
                (None, None) => {}
            }
            previous_highlight = current_highlight;

            let current_focus = focus_target(frame);
            match (previous_focus, current_focus) {
                (Some(_), Some(target)) => {
                    animation.focus_y.spring_to(time, target.y);
                    animation.focus_height.spring_to(time, target.height);
                }
                (None, Some(target)) => {
                    animation.focus_y.set(time, target.y);
                    animation.focus_height.set(time, target.height);
                    animation.focus_intensity.set(time, 0.0);
                    animation.focus_intensity.spring_to(time, 1.0);
                }
                (Some(_), None) => animation.focus_intensity.spring_to(time, 0.0),
                (None, None) => {}
            }
            previous_focus = current_focus;
            index = effective + 1;
        }
        animation
    }
}

fn cursor_target(frame: &PublishedFrame) -> Option<CursorTarget> {
    frame.cursor.as_ref().map(|cursor| CursorTarget {
        x_columns: cursor.char_offset as f32 + cursor.text_length as f32 * 0.5,
        y: cursor.line_index as f32 * LINE_HEIGHT + LINE_HEIGHT,
    })
}

fn highlight_target(frame: &PublishedFrame) -> Option<HighlightTarget> {
    frame
        .bright_ranges
        .first()
        .or_else(|| frame.glows.first())
        .map(|overlay| HighlightTarget {
            x_columns: overlay.start as f32,
            y: overlay.line_index as f32 * LINE_HEIGHT,
            width_columns: overlay.end.saturating_sub(overlay.start) as f32,
        })
}

fn focus_target(frame: &PublishedFrame) -> Option<FocusTarget> {
    let spotlight = frame.spotlight.as_ref()?;
    let mut rows = spotlight
        .ranges
        .iter()
        .filter(|(_, ranges)| !ranges.is_empty())
        .filter_map(|(line_index, _)| line_index.parse::<usize>().ok())
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    rows.sort_unstable();
    let first = rows[0];
    let last = rows[rows.len() - 1];
    Some(FocusTarget {
        y: first as f32 * LINE_HEIGHT,
        height: (last - first + 1) as f32 * LINE_HEIGHT,
    })
}

fn frame_scroll_target(frame: &PublishedFrame) -> f32 {
    const VISIBLE_ROWS: usize = 14;
    if frame.line_order.len() <= VISIBLE_ROWS {
        return 0.0;
    }
    let active_row = frame
        .cursor
        .as_ref()
        .map(|cursor| cursor.line_index)
        .or_else(|| frame.bright_ranges.first().map(|range| range.line_index))
        .or_else(|| frame.glows.first().map(|range| range.line_index))
        .or_else(|| {
            frame.spotlight.as_ref().and_then(|spotlight| {
                spotlight
                    .ranges
                    .keys()
                    .filter_map(|line| line.parse::<usize>().ok())
                    .max()
            })
        })
        .or_else(|| {
            frame.status.as_ref().and_then(|status| {
                status
                    .ranges
                    .keys()
                    .filter_map(|line| line.parse::<usize>().ok())
                    .max()
            })
        })
        .unwrap_or(0);
    let first_visible = active_row
        .saturating_sub(VISIBLE_ROWS / 2)
        .min(frame.line_order.len() - VISIBLE_ROWS);
    first_visible as f32 * LINE_HEIGHT
}

#[derive(Clone, Copy)]
struct CursorTarget {
    x_columns: f32,
    y: f32,
}

#[derive(Clone, Copy)]
struct HighlightTarget {
    x_columns: f32,
    y: f32,
    width_columns: f32,
}

#[derive(Clone, Copy)]
struct FocusTarget {
    y: f32,
    height: f32,
}

struct CodeSample<'a> {
    lines: Vec<PlacedLine<'a>>,
    reveals: Vec<InlineRevealFrame<'a>>,
    squiggles: Vec<SquiggleFrame>,
    annotations: Vec<AnnotationFrame>,
    pointer: PointerFrame,
    focus_y: f32,
    focus_height: f32,
    token_highlight: TokenHighlight,
    bright_text: Vec<BrightTextFrame>,
}

impl PublishedCode {
    fn compile(
        chapter_id: &str,
        section_id: &str,
        slideshow: PublishedSlideshow,
        step_times: Vec<f64>,
    ) -> Result<Self> {
        if slideshow.frames.len() != step_times.len() || slideshow.frames.is_empty() {
            bail!("section '{section_id}' has mismatched frames and step times");
        }
        let step_times = step_times
            .into_iter()
            .map(|time| time as f32)
            .collect::<Vec<_>>();
        let normalized_orders = slideshow
            .frames
            .iter()
            .map(normalized_line_order)
            .collect::<Vec<_>>();
        let mut keys = Vec::<(String, usize)>::new();
        for order in &normalized_orders {
            for key in order {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }

        let mut lines = Vec::new();
        for (source_id, occurrence) in keys {
            let template = slideshow.template.lines.get(&source_id).with_context(|| {
                format!("section '{section_id}' has no template line '{source_id}'")
            })?;
            let id = format!("{chapter_id}/{section_id}/{source_id}#{occurrence}");
            let mut spans = Vec::new();
            let mut variants = Vec::new();
            for part_id in &template.part_order {
                let part = template.parts.get(part_id).with_context(|| {
                    format!("line '{source_id}' has no template part '{part_id}'")
                })?;
                if let Some(versions) = &part.versions {
                    for (version, tokens) in &versions.0 {
                        append_variant(
                            &id,
                            part_id,
                            Some(version.clone()),
                            tokens,
                            &mut spans,
                            &mut variants,
                        );
                    }
                } else {
                    append_variant(&id, part_id, None, &part.tokens, &mut spans, &mut variants);
                }
            }
            lines.push(PreparedLine {
                source_id,
                occurrence,
                line: CodeLine::new(id.clone(), spans),
                y: PropertyId::new(format!("{id}.y")),
                opacity: PropertyId::new(format!("{id}.opacity")),
                blur: PropertyId::new(format!("{id}.blur")),
                variants,
            });
        }
        let initial_frame = step_times
            .partition_point(|time| *time <= 0.0)
            .saturating_sub(1);
        let mut initial_values = Vec::new();
        let mut previous_rows = HashMap::new();
        let mut previous_opacities = HashMap::new();
        let mut previous_targets = HashMap::new();
        for line in &lines {
            let key = (line.source_id.clone(), line.occurrence);
            let row = row_for(&normalized_orders[initial_frame], &key)
                .or_else(|| first_row_for(&normalized_orders, &key))
                .unwrap_or(0);
            let visible = row_for(&normalized_orders[initial_frame], &key).is_some();
            initial_values.push((line.y.clone(), row as f32 * LINE_HEIGHT));
            let opacity = if visible {
                line_opacity(&slideshow.frames[initial_frame], &line.source_id)
            } else {
                0.0
            };
            initial_values.push((line.opacity.clone(), opacity));
            initial_values.push((line.blur.clone(), if visible { 0.0 } else { 4.0 }));
            previous_rows.insert(key.clone(), visible.then_some(row));
            previous_opacities.insert(key.clone(), opacity);
            for variant in &line.variants {
                let target =
                    first_variant_target(line, variant, &slideshow.frames, initial_frame, visible);
                initial_values.push((variant.progress.clone(), target));
                previous_targets.insert(variant.progress.clone(), target);
            }
        }

        let line_profile = SpringProfile::from_visual_duration(0.45, 0.0, 0.001, 0.001);
        let part_profile = SpringProfile::from_visual_duration(0.4, 0.0, 0.001, 0.001);
        let mut events = Vec::new();
        let mut index = initial_frame + 1;
        while index < slideshow.frames.len() {
            let time = step_times[index];
            let mut effective = index;
            while effective + 1 < slideshow.frames.len() && step_times[effective + 1] == time {
                effective += 1;
            }
            let frame = &slideshow.frames[effective];
            let order = &normalized_orders[effective];
            let mut writes = Vec::new();
            for line in &lines {
                let key = (line.source_id.clone(), line.occurrence);
                let row = row_for(order, &key);
                let previous = previous_rows.get(&key).copied().flatten();
                let opacity = if row.is_some() {
                    line_opacity(frame, &line.source_id)
                } else {
                    0.0
                };
                if previous_opacities[&key] != opacity {
                    writes.push(Animation::spring(
                        line.opacity.clone(),
                        opacity,
                        line_profile,
                    ));
                }
                if row.is_some() != previous.is_some() {
                    writes.push(Animation::spring(
                        line.blur.clone(),
                        if row.is_some() { 0.0 } else { 4.0 },
                        line_profile,
                    ));
                }
                if let Some(row) = row
                    && previous != Some(row)
                {
                    writes.push(Animation::spring(
                        line.y.clone(),
                        row as f32 * LINE_HEIGHT,
                        line_profile,
                    ));
                }
                previous_opacities.insert(key.clone(), opacity);
                previous_rows.insert(key, row);
                if row.is_some() {
                    for variant in &line.variants {
                        let target = variant_target(line, variant, frame);
                        let previous = previous_targets[&variant.progress];
                        if target != previous {
                            writes.push(Animation::spring(
                                variant.progress.clone(),
                                target,
                                part_profile,
                            ));
                            previous_targets.insert(variant.progress.clone(), target);
                        }
                    }
                }
            }
            if !writes.is_empty() {
                events.push(Animation::delay(time, Animation::parallel(writes)));
            }
            index = effective + 1;
        }
        let animation = Animation::parallel(events);
        let timeline = Timeline::compile(initial_values, &animation)?;
        let overlays = OverlayAnimation::compile(&slideshow.frames, &step_times, initial_frame);
        Ok(Self {
            lines,
            timeline,
            frames: slideshow.frames,
            step_times,
            overlays,
        })
    }

    fn render(&self, renderer: &mut HeadlessRenderer, time: f32) -> Result<Vec<u8>> {
        let sample = self.sample(renderer, time)?;
        renderer.render_editor(&EditorFrame {
            panel_offset_y: 0.0,
            panel_rotation: 0.0,
            panel_tilt_x: 0.0,
            panel_tilt_y: 0.0,
            panel_scale: 1.0,
            panel_near_blur: 0.0,
            focus_intensity: 0.0,
            focus_line_y: sample.focus_y,
            focus_height: sample.focus_height,
            token_highlight: sample.token_highlight,
            bright_text: &sample.bright_text,
            pointer: sample.pointer,
            inline_reveals: &sample.reveals,
            squiggles: &sample.squiggles,
            annotations: &sample.annotations,
            lines: &sample.lines,
        })
    }

    fn render_type_display(&self, renderer: &mut HeadlessRenderer, time: f32) -> Result<Vec<u8>> {
        let line = &self.lines[0].line;
        let reveals = self.lines[0]
            .variants
            .iter()
            .filter(|variant| variant.start_span < variant.end_span)
            .map(|variant| InlineRevealFrame {
                line_id: line.id.as_str(),
                start_span: variant.start_span,
                end_span: variant.end_span,
                progress: self
                    .timeline
                    .sample(&variant.progress, time)
                    .unwrap()
                    .position
                    .clamp(0.0, 1.0),
            })
            .collect::<Vec<_>>();
        renderer.render_centered_code_line(line, &reveals)
    }

    fn sample<'a>(&'a self, renderer: &mut HeadlessRenderer, time: f32) -> Result<CodeSample<'a>> {
        let frame_index = self
            .step_times
            .partition_point(|step| *step <= time.max(0.0))
            .saturating_sub(1);
        let frame = &self.frames[frame_index];
        let order = normalized_line_order(frame);
        let scroll_y = self.overlays.scroll_y.sample(time).position.max(0.0);
        let viewport_height = 0.70 * HEIGHT as f32 - 104.0;
        let mut lines = self
            .lines
            .iter()
            .map(|line| {
                let y = self.timeline.sample(&line.y, time).unwrap().position - scroll_y;
                let top_opacity = if scroll_y > 0.01 {
                    smoothstep((y / 20.0).clamp(0.0, 1.0))
                } else {
                    1.0
                };
                let bottom_opacity =
                    smoothstep(((viewport_height - LINE_HEIGHT - y) / 20.0).clamp(0.0, 1.0));
                PlacedLine {
                    line: &line.line,
                    x: 0.0,
                    y,
                    opacity: self
                        .timeline
                        .sample(&line.opacity, time)
                        .unwrap()
                        .position
                        .clamp(0.0, 1.0)
                        * top_opacity
                        * bottom_opacity,
                    blur: self
                        .timeline
                        .sample(&line.blur, time)
                        .unwrap()
                        .position
                        .max(0.0),
                }
            })
            .filter(|line| line.opacity > 0.001)
            .collect::<Vec<_>>();
        lines.sort_by(|left, right| left.y.total_cmp(&right.y));
        let mut reveals = Vec::new();
        for line in &self.lines {
            for variant in &line.variants {
                if variant.start_span < variant.end_span {
                    reveals.push(InlineRevealFrame {
                        line_id: line.line.id.as_str(),
                        start_span: variant.start_span,
                        end_span: variant.end_span,
                        progress: self
                            .timeline
                            .sample(&variant.progress, time)
                            .unwrap()
                            .position
                            .clamp(0.0, 1.0),
                    });
                }
            }
        }

        let column_width = renderer.code_column_width();
        let pointer_x = (self.overlays.cursor_x_columns.sample(time).position * column_width
            - 12.0
            + self.overlays.cursor_rest_x.sample(time).position)
            .clamp(0.0, WIDTH as f32 * (0.89 - 0.145) - 56.0);
        let pointer_y = (self.overlays.cursor_y.sample(time).position
            + self.overlays.cursor_rest_y.sample(time).position
            - scroll_y)
            .clamp(0.0, viewport_height - 24.0);
        let pointer = PointerFrame {
            x: pointer_x,
            y: pointer_y,
            opacity: self
                .overlays
                .cursor_opacity
                .sample(time)
                .position
                .clamp(0.0, 1.0),
            rotation: 0.0,
            scale: self.overlays.cursor_scale.sample(time).position.max(0.0) * 24.0 / 36.0,
            blur: self.overlays.cursor_blur.sample(time).position.max(0.0),
        };
        let token_highlight = TokenHighlight {
            x: self.overlays.highlight_x_columns.sample(time).position * column_width,
            y: (self.overlays.highlight_y.sample(time).position - scroll_y)
                .clamp(0.0, viewport_height - LINE_HEIGHT),
            width: (self.overlays.highlight_width_columns.sample(time).position * column_width)
                .max(0.0),
            opacity: self
                .overlays
                .highlight_opacity
                .sample(time)
                .position
                .clamp(0.0, 0.8),
        };
        let focus_intensity = self
            .overlays
            .focus_intensity
            .sample(time)
            .position
            .clamp(0.0, 1.0);
        let focus_height = self
            .overlays
            .focus_height
            .sample(time)
            .position
            .clamp(LINE_HEIGHT, viewport_height);
        let focus_y = (self.overlays.focus_y.sample(time).position - scroll_y)
            .clamp(0.0, viewport_height - focus_height);
        let mut bright_text = Vec::new();
        if let Some(spotlight) = &frame.spotlight
            && focus_intensity > 0.001
        {
            let dim = spotlight.dim.unwrap_or(0.4).clamp(0.0, 1.0);
            for (line_index, ranges) in &spotlight.ranges {
                let Ok(line_index) = line_index.parse::<usize>() else {
                    continue;
                };
                let Some(key) = order.get(line_index) else {
                    continue;
                };
                let prepared = canonical_line(&self.lines, key)?;
                let active_line = visible_code_line(frame, prepared, line_index);
                let text: String = active_line
                    .spans()
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect();
                let y = self.timeline.sample(&prepared.y, time).unwrap().position - scroll_y;
                let structural_opacity = lines
                    .iter()
                    .find(|line| line.line.id == prepared.line.id)
                    .map_or(0.0, |line| line.opacity);
                for range in ranges {
                    let start = utf16_to_byte(&text, range.start);
                    let end = utf16_to_byte(&text, range.end);
                    if start >= end {
                        continue;
                    }
                    let bounds = renderer.measure_text_byte_range(&active_line, start, end)?;
                    bright_text.push(BrightTextFrame {
                        line: active_line.clone(),
                        source_x: bounds.x,
                        width: bounds.width,
                        y,
                        opacity: structural_opacity * focus_intensity,
                        blur: self
                            .timeline
                            .sample(&prepared.blur, time)
                            .unwrap()
                            .position
                            .max(0.0),
                    });
                }
            }
            let dimming = 1.0 - focus_intensity * (1.0 - dim);
            for line in &mut lines {
                line.opacity *= dimming;
            }
        }
        let mut squiggles = Vec::new();
        for overlay in &frame.squiggles {
            if let Some(target) =
                self.measure_overlay(renderer, frame, &order, overlay.clone(), time)?
            {
                squiggles.push(SquiggleFrame {
                    x: target.bounds.x,
                    y: target.line_y,
                    width: target.bounds.width,
                    opacity: 1.0,
                });
            }
        }
        let mut annotations = Vec::new();
        if let Some(status) = &frame.status {
            for (line_index, ranges) in &status.ranges {
                let line_index = line_index.parse().unwrap_or(0);
                for range in ranges {
                    let overlay = PublishedOverlay {
                        line_index,
                        start: range.start,
                        end: range.end,
                    };
                    let Some(target) =
                        self.measure_overlay(renderer, frame, &order, overlay, time)?
                    else {
                        continue;
                    };
                    match range.status.as_str() {
                        "error" | "interrupted" => squiggles.push(SquiggleFrame {
                            x: target.bounds.x,
                            y: target.line_y,
                            width: target.bounds.width,
                            opacity: 1.0,
                        }),
                        "running" => annotations.push(AnnotationFrame {
                            target: TargetGeometry {
                                x: target.bounds.x,
                                width: target.bounds.width,
                                line_y: target.line_y,
                            },
                            effect: AnnotationEffect::FocusPulse,
                            phase: (time * 0.7).fract(),
                        }),
                        "success" => annotations.push(AnnotationFrame {
                            target: TargetGeometry {
                                x: target.bounds.x,
                                width: target.bounds.width,
                                line_y: target.line_y,
                            },
                            effect: AnnotationEffect::PrismaticBloom,
                            phase: ((time
                                - status_started_at(
                                    &self.frames,
                                    &self.step_times,
                                    frame_index,
                                    line_index,
                                    range,
                                ))
                                / 0.9)
                                .clamp(0.0, 1.0),
                        }),
                        _ => {}
                    }
                }
            }
        }
        let burst_phase = (time - self.step_times[frame_index]) / 0.9;
        if (0.0..1.0).contains(&burst_phase) {
            for burst in &frame.bursts {
                if let Some(target) =
                    self.measure_overlay(renderer, frame, &order, burst.as_overlay(), time)?
                {
                    annotations.push(AnnotationFrame {
                        target: TargetGeometry {
                            x: target.bounds.x,
                            width: target.bounds.width,
                            line_y: target.line_y,
                        },
                        effect: burst.effect(),
                        phase: burst_phase,
                    });
                }
            }
        }
        squiggles
            .retain(|squiggle| squiggle.y >= 0.0 && squiggle.y + LINE_HEIGHT <= viewport_height);
        annotations.retain(|annotation| {
            annotation.target.line_y >= 0.0
                && annotation.target.line_y + LINE_HEIGHT <= viewport_height
        });
        Ok(CodeSample {
            lines,
            reveals,
            squiggles,
            annotations,
            pointer,
            focus_y,
            focus_height,
            token_highlight,
            bright_text,
        })
    }

    fn measure_overlay(
        &self,
        renderer: &mut HeadlessRenderer,
        frame: &PublishedFrame,
        order: &[(String, usize)],
        overlay: PublishedOverlay,
        time: f32,
    ) -> Result<Option<MeasuredOverlay>> {
        let Some(key) = order.get(overlay.line_index) else {
            return Ok(None);
        };
        let prepared = canonical_line(&self.lines, key)?;
        let visible = visible_line(frame, prepared)?;
        let start = utf16_to_byte(&visible, overlay.start);
        let end = utf16_to_byte(&visible, overlay.end);
        if start >= end {
            return Ok(None);
        }
        let measure_line = CodeLine::new(
            "measure",
            vec![StyledSpan::new(visible, SyntaxStyle::Plain)],
        );
        let bounds = renderer.measure_text_byte_range(&measure_line, start, end)?;
        let sampled_y = self.timeline.sample(&prepared.y, time).unwrap().position
            - self.overlays.scroll_y.sample(time).position.max(0.0);
        Ok(Some(MeasuredOverlay {
            bounds,
            line_y: sampled_y,
        }))
    }
}

struct MeasuredOverlay {
    bounds: TextRangeBounds,
    line_y: f32,
}

fn append_variant(
    line_id: &str,
    part_id: &str,
    version: Option<String>,
    tokens: &[PublishedToken],
    spans: &mut Vec<StyledSpan>,
    variants: &mut Vec<PreparedVariant>,
) {
    let start_span = spans.len();
    spans.extend(
        tokens
            .iter()
            .filter(|token| !token.text.is_empty())
            .map(|token| StyledSpan::new(&token.text, token_style(token.color.as_deref()))),
    );
    let end_span = spans.len();
    variants.push(PreparedVariant {
        part_id: part_id.to_owned(),
        version: version.clone(),
        start_span,
        end_span,
        progress: PropertyId::new(format!(
            "{line_id}.part.{part_id}.{}",
            version.as_deref().unwrap_or("default")
        )),
    });
}

fn token_style(color: Option<&str>) -> SyntaxStyle {
    let Some(color) = color.and_then(|color| color.strip_prefix('#')) else {
        return SyntaxStyle::Plain;
    };
    if color.len() != 6 {
        return SyntaxStyle::Plain;
    }
    let parse = |range| u8::from_str_radix(&color[range], 16).ok();
    match (parse(0..2), parse(2..4), parse(4..6)) {
        (Some(red), Some(green), Some(blue)) => SyntaxStyle::Rgb(red, green, blue),
        _ => SyntaxStyle::Plain,
    }
}

fn first_variant_target(
    line: &PreparedLine,
    variant: &PreparedVariant,
    frames: &[PublishedFrame],
    initial_frame: usize,
    visible: bool,
) -> f32 {
    if visible {
        return variant_target(line, variant, &frames[initial_frame]);
    }
    frames
        .iter()
        .find(|frame| frame.state.contains_key(&line.source_id))
        .map_or(0.0, |frame| variant_target(line, variant, frame))
}

fn variant_target(line: &PreparedLine, variant: &PreparedVariant, frame: &PublishedFrame) -> f32 {
    let Some(state) = frame.state.get(&line.source_id) else {
        return 0.0;
    };
    let Some(value) = state.parts.get(&variant.part_id) else {
        return 0.0;
    };
    match (value, variant.version.as_deref()) {
        (PublishedPartState::Bool(true), None) => 1.0,
        (PublishedPartState::String(active), Some(version)) if active == version => 1.0,
        (PublishedPartState::String(active), None) if !active.is_empty() => 1.0,
        _ => 0.0,
    }
}

fn line_opacity(frame: &PublishedFrame, line_id: &str) -> f32 {
    if frame.state.get(line_id).is_some_and(|state| state.dimmed) {
        0.4
    } else {
        1.0
    }
}

fn normalized_line_order(frame: &PublishedFrame) -> Vec<(String, usize)> {
    let mut occurrences = HashMap::<&str, usize>::new();
    frame
        .line_order
        .iter()
        .map(|id| {
            let occurrence = occurrences.entry(id).or_default();
            let key = (id.clone(), *occurrence);
            *occurrence += 1;
            key
        })
        .collect()
}

fn row_for(order: &[(String, usize)], key: &(String, usize)) -> Option<usize> {
    order.iter().position(|candidate| candidate == key)
}

fn first_row_for(orders: &[Vec<(String, usize)>], key: &(String, usize)) -> Option<usize> {
    orders.iter().find_map(|order| row_for(order, key))
}

fn canonical_line<'a>(
    lines: &'a [PreparedLine],
    key: &(String, usize),
) -> Result<&'a PreparedLine> {
    lines
        .iter()
        .find(|line| line.source_id == key.0 && line.occurrence == key.1)
        .with_context(|| format!("no canonical line for '{}#{}'", key.0, key.1))
}

fn visible_line(frame: &PublishedFrame, line: &PreparedLine) -> Result<String> {
    frame
        .state
        .get(&line.source_id)
        .context("visible line has no frame state")?;
    let mut output = String::new();
    for variant in &line.variants {
        if variant_target(line, variant, frame) <= 0.5 {
            continue;
        }
        let text = line.line.spans()[variant.start_span..variant.end_span]
            .iter()
            .map(|span| span.text.as_str())
            .collect::<String>();
        output.push_str(&text);
    }
    Ok(output)
}

fn visible_code_line(frame: &PublishedFrame, line: &PreparedLine, line_index: usize) -> CodeLine {
    let mut spans = Vec::new();
    for variant in &line.variants {
        if variant_target(line, variant, frame) > 0.5 {
            spans.extend_from_slice(&line.line.spans()[variant.start_span..variant.end_span]);
        }
    }
    CodeLine::new(
        format!("spotlight:{}:{line_index}", line.line.id.as_str()),
        spans,
    )
}

fn utf16_to_byte(text: &str, units: usize) -> usize {
    let mut consumed = 0;
    for (byte, character) in text.char_indices() {
        if consumed >= units {
            return byte;
        }
        consumed += character.len_utf16();
    }
    text.len()
}

struct PublishedComponent {
    flow: PublishedComponentFlow,
    step_times: Vec<f64>,
    task_tracks: HashMap<String, StateTrack<TaskState>>,
    bool_tracks: HashMap<String, AnalyticTrack>,
}

impl PublishedComponent {
    fn compile(
        flow: PublishedComponentFlow,
        step_times: Vec<f64>,
        duration: Duration,
    ) -> Result<Self> {
        let keys = flow
            .snapshots
            .iter()
            .flat_map(|snapshot| component_nodes(&flow.name, snapshot))
            .map(|node| node.key)
            .collect::<HashSet<_>>();
        let mut task_tracks = HashMap::new();
        for key in keys {
            let states = flow
                .snapshots
                .iter()
                .map(|snapshot| {
                    component_nodes(&flow.name, snapshot)
                        .into_iter()
                        .find(|node| node.key == key)
                        .map_or(TaskState::Hidden, |node| {
                            component_task_state(
                                &node.state,
                                node.result.as_deref(),
                                node.error.as_deref(),
                            )
                        })
                })
                .collect::<Vec<_>>();
            let initial_at = f64::from(step_times[0] as f32);
            let track = StateTrack::compile_at(
                initial_at,
                states[0].clone(),
                step_times
                    .iter()
                    .zip(states)
                    .skip(1)
                    .map(|(at, state)| TimedState::new(f64::from(*at as f32), state)),
                f64::from(duration.as_seconds() as f32),
            )?;
            task_tracks.insert(key, track);
        }
        let mut bool_tracks = HashMap::new();
        for (key, visual_duration, bounce) in [
            ("inputVisible", 0.4, 0.1),
            ("showLog1", 0.3, 0.0),
            ("showLog2", 0.3, 0.0),
            ("showProgramLog", 0.3, 0.0),
        ] {
            if flow
                .snapshots
                .iter()
                .any(|snapshot| snapshot.contains_key(key))
            {
                bool_tracks.insert(
                    key.to_owned(),
                    component_bool_track(&flow, &step_times, key, visual_duration, bounce),
                );
            }
        }
        Ok(Self {
            flow,
            step_times,
            task_tracks,
            bool_tracks,
        })
    }

    fn bool_value(&self, key: &str, time: f32) -> f32 {
        self.bool_tracks
            .get(key)
            .map_or(0.0, |track| track.sample(time).position)
    }

    fn composite(
        &self,
        renderer: &mut HeadlessRenderer,
        pixels: &mut [u8],
        time: f32,
    ) -> Result<()> {
        if self.flow.name == "EffectTypeDisplay" {
            return Ok(());
        }
        let current = self
            .step_times
            .partition_point(|step| *step <= f64::from(time.max(0.0)))
            .saturating_sub(1);
        let snapshot = &self.flow.snapshots[current];
        let input_opacity = if self.flow.name == "ErrorShortCircuitDemo" {
            self.bool_value("inputVisible", time).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if input_opacity > 0.001 {
            let display_snapshot = if snapshot
                .get("inputVisible")
                .and_then(PublishedValue::as_bool)
                .unwrap_or(false)
            {
                snapshot
            } else {
                self.flow.snapshots[..current]
                    .iter()
                    .rfind(|snapshot| {
                        snapshot
                            .get("inputVisible")
                            .and_then(PublishedValue::as_bool)
                            .unwrap_or(false)
                    })
                    .unwrap_or(snapshot)
            };
            composite_error_input(
                renderer,
                pixels,
                display_snapshot,
                time - self.step_times[current] as f32,
                input_opacity,
            );
        }
        let specs = component_nodes(&self.flow.name, snapshot);
        if specs.is_empty() {
            return Ok(());
        }
        let count = specs.len() as f32;
        let spacing = if specs.len() == 2 { 520.0 } else { 400.0 };
        let left = WIDTH as f32 * 0.5 - (count - 1.0) * spacing * 0.5;
        if self.flow.name == "WhyComposeDemo"
            && snapshot
                .get("showContainer")
                .and_then(PublishedValue::as_bool)
                .unwrap_or(false)
            && !snapshot
                .get("combined")
                .and_then(PublishedValue::as_bool)
                .unwrap_or(false)
        {
            let width = 1_020;
            stroke_rect(
                pixels,
                (WIDTH as i32 - width) / 2,
                660,
                width,
                250,
                [71, 85, 105, 190],
            );
        }
        let mut owned = Vec::new();
        for (index, spec) in specs.into_iter().enumerate() {
            let track = &self.task_tracks[&spec.key];
            let sampled = track.sample(time);
            let state = sampled.current.clone();
            let result_width = match &state {
                TaskState::Succeeded(Some(result)) => {
                    Some(renderer.measure_task_result_width(result))
                }
                _ => None,
            };
            owned.push(OwnedTaskFrame {
                id: TaskId::new(format!("{}.{}", self.flow.name, spec.key)),
                name: spec.name,
                state,
                previous_state: sampled.previous.clone(),
                previous_state_duration: sampled.previous_duration,
                state_age: sampled.age,
                visible_age: time
                    - track
                        .last_interval_start(time, |state| state != &TaskState::Hidden)
                        .unwrap_or(sampled.transition_at),
                result_width,
                x: component_node_x(&self.flow.name, &spec.key, left, index, spacing),
            });
        }
        let frames = owned
            .iter()
            .map(|node| TaskFrame {
                id: &node.id,
                x: node.x,
                y: 790.0,
                x_velocity: 0.0,
                y_velocity: 0.0,
                name: &node.name,
                result_width: node.result_width,
                previous_state: &node.previous_state,
                previous_state_duration: node.previous_state_duration,
                state: &node.state,
                state_age: node.state_age,
                visible_age: node.visible_age,
            })
            .collect::<Vec<_>>();
        let links = if self.flow.name == "ErrorShortCircuitDemo"
            && owned.len() == 3
            && owned
                .iter()
                .any(|node| !matches!(node.state, TaskState::Hidden))
        {
            vec![TaskLinkFrame {
                from: [owned[1].x + 90.0, 790.0],
                to: [owned[2].x - 90.0, 790.0],
                pulse: None,
            }]
        } else {
            Vec::new()
        };
        renderer.composite_task_scene(
            pixels,
            &TaskSceneFrame {
                quote: None,
                links: &links,
                nodes: &frames,
            },
        )?;
        let logs: &[(&str, &str, &str)] = match self.flow.name.as_str() {
            "EffectSyncDemo" => &[
                ("showLog1", "state1", "I'M TRAPPED IN A THUNK!"),
                ("showLog2", "state2", "I'M TRAPPED IN A THUNK!"),
            ],
            "WhyComposeDemo" => &[
                ("showLog2", "state2", "ALL CLEAR!"),
                ("showProgramLog", "programState", "ALL CLEAR!"),
            ],
            _ => &[],
        };
        for (visible_key, node_key, message) in logs {
            let opacity = self.bool_value(visible_key, time).clamp(0.0, 1.0);
            if opacity <= 0.001 {
                continue;
            }
            if let Some(node) = owned
                .iter()
                .find(|node| node.id.as_str().ends_with(node_key))
            {
                renderer.composite_centered_text(
                    pixels,
                    message,
                    [node.x, 675.0 - (1.0 - opacity) * 12.0],
                    22.0,
                    [147, 197, 253],
                    opacity,
                );
            }
        }
        Ok(())
    }
}

struct OwnedTaskFrame {
    id: TaskId,
    name: String,
    state: TaskState,
    previous_state: TaskState,
    previous_state_duration: f32,
    state_age: f32,
    visible_age: f32,
    result_width: Option<f32>,
    x: f32,
}

struct ComponentNode {
    key: String,
    name: String,
    state: String,
    result: Option<String>,
    error: Option<String>,
}

fn component_bool_track(
    flow: &PublishedComponentFlow,
    times: &[f64],
    key: &str,
    visual_duration: f32,
    bounce: f32,
) -> AnalyticTrack {
    let initial = flow.snapshots[0]
        .get(key)
        .and_then(PublishedValue::as_bool)
        .unwrap_or(false);
    let mut track = AnalyticTrack::new(
        &format!("component.{key}"),
        f32::from(initial),
        visual_duration,
        bounce,
    );
    let mut previous = initial;
    for (index, snapshot) in flow.snapshots.iter().enumerate().skip(1) {
        let current = snapshot
            .get(key)
            .and_then(PublishedValue::as_bool)
            .unwrap_or(false);
        if current != previous {
            track.spring_to(times[index] as f32, f32::from(current));
            previous = current;
        }
    }
    track
}

fn component_task_state(state: &str, result: Option<&str>, error: Option<&str>) -> TaskState {
    match state {
        "running" => TaskState::Running,
        "completed" => TaskState::Succeeded(result.map(str::to_owned)),
        "failed" => TaskState::Failed(error.unwrap_or("Failed").to_owned()),
        "interrupted" => TaskState::Failed("Interrupted".to_owned()),
        "hidden" => TaskState::Hidden,
        _ => TaskState::Idle,
    }
}

fn composite_error_input(
    renderer: &mut HeadlessRenderer,
    pixels: &mut [u8],
    snapshot: &HashMap<String, PublishedValue>,
    state_age: f32,
    opacity: f32,
) {
    let error = snapshot
        .get("inputError")
        .and_then(PublishedValue::as_bool)
        .unwrap_or(false);
    let shake = snapshot
        .get("inputShake")
        .and_then(PublishedValue::as_bool)
        .unwrap_or(false);
    let pressed = snapshot
        .get("enterPressed")
        .and_then(PublishedValue::as_bool)
        .unwrap_or(false);
    let text = snapshot
        .get("typedText")
        .and_then(PublishedValue::as_str)
        .unwrap_or("");
    let decay = (1.0 - state_age.max(0.0) / 0.3).clamp(0.0, 1.0);
    let shake_x = if shake {
        (state_age.max(0.0) * 70.0).sin() * 12.0 * decay
    } else {
        0.0
    };
    let field_x = (WIDTH as f32 * 0.5 - 80.0 + shake_x).round() as i32;
    let field_y = (778.0 + (1.0 - opacity) * 18.0).round() as i32;
    fill_rect(
        pixels,
        field_x - 150,
        field_y - 34,
        300,
        68,
        if error {
            faded([69, 16, 20, 235], opacity)
        } else {
            faded([20, 24, 32, 235], opacity)
        },
    );
    stroke_rect(
        pixels,
        field_x - 150,
        field_y - 34,
        300,
        68,
        if error {
            faded([239, 68, 68, 210], opacity)
        } else {
            faded([119, 128, 144, 110], opacity)
        },
    );
    let key_x = field_x + 246;
    fill_rect(
        pixels,
        key_x - 58,
        field_y - 30 + i32::from(pressed) * 4,
        116,
        60,
        if pressed {
            faded([50, 56, 68, 255], opacity)
        } else {
            faded([28, 33, 43, 255], opacity)
        },
    );
    stroke_rect(
        pixels,
        key_x - 58,
        field_y - 30 + i32::from(pressed) * 4,
        116,
        60,
        faded([119, 128, 144, 150], opacity),
    );
    renderer.composite_centered_text(
        pixels,
        "Age:",
        [field_x as f32 - 205.0, field_y as f32],
        24.0,
        [165, 173, 186],
        opacity,
    );
    renderer.composite_centered_text(
        pixels,
        &format!("{text}|"),
        [field_x as f32, field_y as f32],
        28.0,
        [235, 238, 243],
        opacity,
    );
    renderer.composite_centered_text(
        pixels,
        "Enter",
        [key_x as f32, field_y as f32 + f32::from(pressed) * 4.0],
        22.0,
        [220, 225, 234],
        opacity,
    );
    if error {
        renderer.composite_centered_text(
            pixels,
            "What are you doing?",
            [field_x as f32, field_y as f32 - 66.0],
            20.0,
            [248, 113, 113],
            opacity,
        );
    }
}

fn faded(mut color: [u8; 4], opacity: f32) -> [u8; 4] {
    color[3] = (f32::from(color[3]) * opacity.clamp(0.0, 1.0)).round() as u8;
    color
}

fn fill_rect(pixels: &mut [u8], x: i32, y: i32, width: i32, height: i32, color: [u8; 4]) {
    for row in y.max(0)..(y + height).min(HEIGHT as i32) {
        for column in x.max(0)..(x + width).min(WIDTH as i32) {
            let offset = (row as usize * WIDTH as usize + column as usize) * 4;
            let alpha = f32::from(color[3]) / 255.0;
            for channel in 0..3 {
                pixels[offset + channel] = (f32::from(color[channel]) * alpha
                    + f32::from(pixels[offset + channel]) * (1.0 - alpha))
                    .round() as u8;
            }
        }
    }
}

fn stroke_rect(pixels: &mut [u8], x: i32, y: i32, width: i32, height: i32, color: [u8; 4]) {
    fill_rect(pixels, x, y, width, 2, color);
    fill_rect(pixels, x, y + height - 2, width, 2, color);
    fill_rect(pixels, x, y, 2, height, color);
    fill_rect(pixels, x + width - 2, y, 2, height, color);
}

fn component_nodes(
    component: &str,
    snapshot: &HashMap<String, PublishedValue>,
) -> Vec<ComponentNode> {
    let visible = |key: &str| {
        snapshot
            .get(key)
            .and_then(PublishedValue::as_bool)
            .unwrap_or(true)
    };
    let state = |key: &str| {
        snapshot
            .get(key)
            .and_then(PublishedValue::as_str)
            .unwrap_or("idle")
            .to_owned()
    };
    let mode = snapshot
        .get("mode")
        .and_then(PublishedValue::as_str)
        .unwrap_or("");
    let node = |key: &str, name: &str, result: Option<&str>, error: Option<&str>| ComponentNode {
        key: key.to_owned(),
        name: name.to_owned(),
        state: state(key),
        result: result.map(str::to_owned),
        error: error.map(str::to_owned),
    };
    match component {
        "EffectIsDescriptionDemo" => {
            let mut task = node(
                "visualState",
                "getTime",
                snapshot.get("result").and_then(PublishedValue::as_str),
                None,
            );
            if !visible("show") {
                task.state = "hidden".to_owned();
            }
            vec![task]
        }
        "EffectRunDemo" => {
            let mut sync = node("syncState", "magicWord", Some("pismire"), None);
            let mut asynchronous = node("asyncState", "asyncWord", Some("pismire"), None);
            if mode == "async" {
                sync.state = "hidden".to_owned();
            } else {
                asynchronous.state = "hidden".to_owned();
            }
            vec![sync, asynchronous]
        }
        "EffectFailDemo" => {
            let mut sync = node("syncState", "boom", None, Some("too chonky"));
            let mut asynchronous = node("asyncState", "slowBoom", None, Some("too chonky"));
            if mode == "async" {
                sync.state = "hidden".to_owned();
            } else {
                asynchronous.state = "hidden".to_owned();
            }
            if !visible("show") {
                sync.state = "hidden".to_owned();
                asynchronous.state = "hidden".to_owned();
            }
            vec![sync, asynchronous]
        }
        "EffectSyncDemo" => {
            let mut first = node("state1", "now", Some("2026-07-15T09:41:00Z"), None);
            let mut second = node("state2", "later", Some("2026-07-15T09:42:00Z"), None);
            if !visible("show1") {
                first.state = "hidden".to_owned();
            }
            if !visible("show2") {
                second.state = "hidden".to_owned();
            }
            vec![first, second]
        }
        "SyncVsSucceedDemo" => ["run1State", "run2State", "run3State"]
            .into_iter()
            .enumerate()
            .map(|(index, key)| {
                let names = ["now", "later", "tomorrow"];
                let results = if mode == "sync" {
                    ["09:41:00", "09:42:00", "09:43:00"]
                } else {
                    ["09:41:00", "09:41:00", "09:41:00"]
                };
                let mut task = node(key, names[index], Some(results[index]), None);
                if task.state == "idle" {
                    task.state = "hidden".to_owned();
                }
                task
            })
            .collect(),
        "EffectPromiseDemo" => {
            let mut task = node("state", "sensorData", Some("{ mSv: 847 }"), None);
            if !visible("show") {
                task.state = "hidden".to_owned();
            }
            vec![task]
        }
        "StructuredConcurrencyDemo" => hide_component_nodes(
            vec![
                node("tree", "tree", Some("oak"), Some("Interrupted")),
                node("star", "star", None, Some("DoneGoneSupernova")),
                node("beetle", "beetle", Some("dung"), Some("Interrupted")),
            ],
            !visible("visible"),
        ),
        "GeneratorsDemo" => hide_component_nodes(
            vec![
                node("sensor", "sensorData", Some("{ mSv: 0.3 }"), None),
                node("bio", "biometrics", Some("{ pulse: 72 }"), None),
                node("analyze", "analyze", None, None),
            ],
            !visible("visible"),
        ),
        "GetUserDemo" => hide_component_nodes(
            vec![
                node("askName", "askName", Some("Olive"), Some("NoSams")),
                node("askAge", "askAge", Some("37"), Some("BadNumber")),
                node(
                    "getUser",
                    "getUser",
                    Some("{ name: Olive, age: 37 }"),
                    Some("InvalidUser"),
                ),
            ],
            !visible("visible"),
        ),
        "ErrorShortCircuitDemo" => hide_component_nodes(
            vec![
                node("askName", "askName", Some("Olive"), Some("NoSams")),
                node("askAge", "askAge", Some("37"), Some("BadNumber")),
                node(
                    "getUser",
                    "getUser",
                    Some("Olive, 37"),
                    Some(match mode {
                        "fail-name" => "NoSams",
                        "fail-age" => "BadNumber",
                        _ => "ShortCircuit",
                    }),
                ),
            ],
            !visible("visible"),
        ),
        "WhyComposeDemo" => {
            let collapsed = visible("combined") || visible("showProgramNode");
            let mut root = node("programState", "program", Some("complete"), None);
            let mut sensor = node("state1", "sensorData", Some("{ mSv: 0.3 }"), None);
            let mut shout = node("state2", "shout", Some("CLEAR!"), None);
            if collapsed {
                sensor.state = "hidden".to_owned();
                shout.state = "hidden".to_owned();
            } else {
                root.state = "hidden".to_owned();
            }
            if !visible("show") {
                root.state = "hidden".to_owned();
                sensor.state = "hidden".to_owned();
                shout.state = "hidden".to_owned();
            }
            vec![root, sensor, shout]
        }
        _ => Vec::new(),
    }
}

fn hide_component_nodes(mut nodes: Vec<ComponentNode>, hidden: bool) -> Vec<ComponentNode> {
    if hidden {
        for node in &mut nodes {
            node.state = "hidden".to_owned();
        }
    }
    nodes
}

fn component_node_x(
    component: &str,
    key: &str,
    default_left: f32,
    index: usize,
    spacing: f32,
) -> f32 {
    let center = WIDTH as f32 * 0.5;
    match (component, key) {
        ("EffectRunDemo" | "EffectFailDemo", _) => center,
        ("WhyComposeDemo", "programState") => center,
        ("WhyComposeDemo", "state1") => center - 260.0,
        ("WhyComposeDemo", "state2") => center + 260.0,
        _ => default_left + index as f32 * spacing,
    }
}

fn type_display_slideshow(component: &PublishedComponent) -> (PublishedSlideshow, Vec<f64>) {
    let tokens = |text: &str, color: &str| PublishedToken {
        text: text.to_owned(),
        color: Some(color.to_owned()),
    };
    let parts = BTreeMap::from([
        (
            "prefix".to_owned(),
            PublishedTemplatePart::new(vec![tokens("Effect<", "#e4e4e7")]),
        ),
        (
            "placeholder".to_owned(),
            PublishedTemplatePart::new(vec![tokens("?", "#71717a")]),
        ),
        (
            "success".to_owned(),
            PublishedTemplatePart::new(vec![tokens("User", "#4ade80")]),
        ),
        (
            "error".to_owned(),
            PublishedTemplatePart::new(vec![tokens(", NotFound", "#f87171")]),
        ),
        (
            "requirements".to_owned(),
            PublishedTemplatePart::new(vec![tokens(", Database", "#60a5fa")]),
        ),
        (
            "suffix".to_owned(),
            PublishedTemplatePart::new(vec![tokens(">", "#e4e4e7")]),
        ),
    ]);
    let frames = component
        .flow
        .snapshots
        .iter()
        .map(|snapshot| PublishedFrame {
            line_order: vec!["type".to_owned()],
            state: HashMap::from([(
                "type".to_owned(),
                PublishedLineState {
                    parts: HashMap::from([
                        ("prefix".to_owned(), PublishedPartState::Bool(true)),
                        (
                            "placeholder".to_owned(),
                            PublishedPartState::Bool(
                                !snapshot
                                    .get("showSuccess")
                                    .and_then(PublishedValue::as_bool)
                                    .unwrap_or(false),
                            ),
                        ),
                        (
                            "success".to_owned(),
                            PublishedPartState::Bool(
                                snapshot
                                    .get("showSuccess")
                                    .and_then(PublishedValue::as_bool)
                                    .unwrap_or(false),
                            ),
                        ),
                        (
                            "error".to_owned(),
                            PublishedPartState::Bool(
                                snapshot
                                    .get("showError")
                                    .and_then(PublishedValue::as_bool)
                                    .unwrap_or(false),
                            ),
                        ),
                        (
                            "requirements".to_owned(),
                            PublishedPartState::Bool(
                                snapshot
                                    .get("showRequirements")
                                    .and_then(PublishedValue::as_bool)
                                    .unwrap_or(false),
                            ),
                        ),
                        ("suffix".to_owned(), PublishedPartState::Bool(true)),
                    ]),
                    dimmed: false,
                },
            )]),
            cursor: None,
            spotlight: None,
            glows: Vec::new(),
            squiggles: Vec::new(),
            bright_ranges: Vec::new(),
            bursts: Vec::new(),
            status: None,
            sound: None,
        })
        .collect();
    (
        PublishedSlideshow {
            template: PublishedTemplate {
                lines: HashMap::from([(
                    "type".to_owned(),
                    PublishedTemplateLine {
                        part_order: vec![
                            "prefix".to_owned(),
                            "placeholder".to_owned(),
                            "success".to_owned(),
                            "error".to_owned(),
                            "requirements".to_owned(),
                            "suffix".to_owned(),
                        ],
                        parts,
                    },
                )]),
            },
            frames,
        },
        component.step_times.clone(),
    )
}

fn smoothstep(value: f32) -> f32 {
    value * value * (3.0 - 2.0 * value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedManifest {
    id: String,
    title: String,
    sections: Vec<PublishedManifestSection>,
    groups: Vec<PublishedManifestGroup>,
}

#[derive(Deserialize)]
struct PublishedManifestSection {
    id: String,
    file: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedManifestGroup {
    id: String,
    title: String,
    section_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedSectionJson {
    id: String,
    audio_file: Option<String>,
    slideshow_data: Option<PublishedSlideshow>,
    step_times: Option<Vec<f64>>,
    component_flow: Option<PublishedComponentFlow>,
    component_step_times: Option<Vec<f64>>,
}

#[derive(Deserialize)]
struct PublishedSlideshow {
    template: PublishedTemplate,
    frames: Vec<PublishedFrame>,
}

#[derive(Deserialize)]
struct PublishedTemplate {
    lines: HashMap<String, PublishedTemplateLine>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedTemplateLine {
    parts: BTreeMap<String, PublishedTemplatePart>,
    part_order: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedTemplatePart {
    tokens: Vec<PublishedToken>,
    versions: Option<OrderedVersions>,
}

struct OrderedVersions(Vec<(String, Vec<PublishedToken>)>);

impl<'de> Deserialize<'de> for OrderedVersions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OrderedVersionsVisitor;

        impl<'de> Visitor<'de> for OrderedVersionsVisitor {
            type Value = OrderedVersions;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an ordered map of inline part versions")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut versions = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some(entry) = map.next_entry()? {
                    versions.push(entry);
                }
                Ok(OrderedVersions(versions))
            }
        }

        deserializer.deserialize_map(OrderedVersionsVisitor)
    }
}

impl PublishedTemplatePart {
    fn new(tokens: Vec<PublishedToken>) -> Self {
        Self {
            tokens,
            versions: None,
        }
    }
}

#[derive(Deserialize)]
struct PublishedToken {
    text: String,
    color: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedFrame {
    line_order: Vec<String>,
    state: HashMap<String, PublishedLineState>,
    cursor: Option<PublishedCursor>,
    spotlight: Option<PublishedSpotlight>,
    #[serde(default)]
    glows: Vec<PublishedOverlay>,
    #[serde(default)]
    squiggles: Vec<PublishedOverlay>,
    #[serde(default)]
    bright_ranges: Vec<PublishedOverlay>,
    #[serde(default)]
    bursts: Vec<PublishedBurst>,
    status: Option<PublishedStatus>,
    sound: Option<String>,
}

#[derive(Clone, Deserialize)]
struct PublishedLineState {
    parts: HashMap<String, PublishedPartState>,
    #[serde(default)]
    dimmed: bool,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum PublishedPartState {
    Bool(bool),
    String(String),
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedCursor {
    line_index: usize,
    char_offset: usize,
    text_length: usize,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedOverlay {
    line_index: usize,
    start: usize,
    end: usize,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedBurst {
    line_index: usize,
    char_offset: usize,
    text_length: usize,
    emoji: Vec<String>,
}

#[derive(Clone, Deserialize)]
struct PublishedStatus {
    ranges: HashMap<String, Vec<PublishedStatusRange>>,
}

#[derive(Clone, Deserialize, PartialEq)]
struct PublishedStatusRange {
    start: usize,
    end: usize,
    status: String,
}

fn status_started_at(
    frames: &[PublishedFrame],
    step_times: &[f32],
    frame_index: usize,
    line_index: usize,
    range: &PublishedStatusRange,
) -> f32 {
    let key = line_index.to_string();
    let mut start = frame_index;
    while start > 0
        && frames[start - 1]
            .status
            .as_ref()
            .and_then(|status| status.ranges.get(&key))
            .is_some_and(|ranges| ranges.iter().any(|candidate| candidate == range))
    {
        start -= 1;
    }
    step_times[start]
}

impl PublishedBurst {
    fn as_overlay(&self) -> PublishedOverlay {
        PublishedOverlay {
            line_index: self.line_index,
            start: self.char_offset,
            end: self.char_offset + self.text_length,
        }
    }

    fn effect(&self) -> AnnotationEffect {
        if self.emoji.iter().any(|emoji| emoji.contains('\u{1f622}')) {
            AnnotationEffect::SadPulse
        } else if self.emoji.iter().any(|emoji| {
            emoji.contains('\u{1f389}') || emoji.contains('\u{2705}') || emoji.contains('\u{2702}')
        }) {
            AnnotationEffect::PrismaticBloom
        } else {
            AnnotationEffect::DangerPulse
        }
    }
}

#[derive(Clone, Deserialize)]
struct PublishedRange {
    start: usize,
    end: usize,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PublishedSpotlight {
    ranges: HashMap<String, Vec<PublishedRange>>,
    dim: Option<f32>,
}

#[derive(Deserialize)]
struct PublishedComponentFlow {
    name: String,
    snapshots: Vec<HashMap<String, PublishedValue>>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum PublishedValue {
    Bool(bool),
    String(String),
}

impl PublishedValue {
    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            Self::String(_) => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Bool(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use kinograph::composition::MediaRole;

    use super::{
        AnnotationEffect, PublishedChapter, TaskState, component_nodes, frame_scroll_target,
    };

    fn published_code(chapter: &str, section: &str) -> super::PublishedCode {
        let directory = Path::new(super::WORKSPACE_ROOT)
            .join("assets/effect-institute")
            .join(chapter);
        let manifest: super::PublishedManifest =
            serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap())
                .unwrap();
        let file = &manifest
            .sections
            .iter()
            .find(|entry| entry.id == section)
            .unwrap()
            .file;
        let raw: super::PublishedSectionJson =
            serde_json::from_slice(&std::fs::read(directory.join(file)).unwrap()).unwrap();
        assert_eq!(raw.id, section);
        super::PublishedCode::compile(
            chapter,
            section,
            raw.slideshow_data.unwrap(),
            raw.step_times.unwrap(),
        )
        .unwrap()
    }

    #[test]
    #[ignore = "requires headless GPU and fonts; bounded published overlay/scroll artifact proof"]
    fn published_overlay_pixels_are_deterministic_and_select_the_expected_lines() {
        let mut renderer = pollster::block_on(super::HeadlessRenderer::new(super::RenderSpec {
            width: super::WIDTH,
            height: super::HEIGHT,
            font_path: super::FONT_PATH.into(),
            file_name: "published-overlay-proof".into(),
        }))
        .unwrap();
        let output =
            std::env::var_os("KINOGRAPH_PUBLISHED_ARTIFACTS").map(std::path::PathBuf::from);
        if let Some(directory) = &output {
            std::fs::create_dir_all(directory).unwrap();
        }
        for (chapter, section, step, row, source_id, expected_text) in [
            (
                "intro",
                "promises-only-happy-path",
                1,
                0,
                "sig",
                "async function checkout(cartId: string): Promise<Order> {",
            ),
            (
                "intro",
                "promises-only-happy-path",
                2,
                0,
                "sig",
                "async function checkout(cartId: string): Promise<Order> {",
            ),
            (
                "basics",
                "effect-sync",
                10,
                7,
                "run1",
                "const now = await Effect.runPromise(getDate)",
            ),
            (
                "basics",
                "effect-sync",
                11,
                7,
                "run1",
                "const now = await Effect.runPromise(getDate)",
            ),
            (
                "intro",
                "effect-shows-errors",
                9,
                1,
                "sig",
                "const slowDie: Effect.Effect<number> =",
            ),
            (
                "intro",
                "effect-shows-errors",
                10,
                1,
                "sig",
                "const slowDie: Effect.Effect<number> =",
            ),
            (
                "intro",
                "abort-signal-infusion",
                6,
                5,
                "abortSignal",
                "    abortSignal: signal",
            ),
            (
                "intro",
                "abort-signal-infusion",
                12,
                11,
                "starCall",
                "    ask(\"Zubenelgenubi\", ticker),",
            ),
        ] {
            let code = published_code(chapter, section);
            let time = code.step_times[step] + 0.12;
            let frame = &code.frames[step];
            let order = super::normalized_line_order(frame);
            let key = &order[row];
            assert_eq!(key, &(source_id.to_owned(), 0));
            assert_eq!(
                super::visible_line(frame, super::canonical_line(&code.lines, key).unwrap())
                    .unwrap(),
                expected_text
            );
            let selected = code
                .measure_overlay(
                    &mut renderer,
                    frame,
                    &order,
                    super::PublishedOverlay {
                        line_index: row,
                        start: 0,
                        end: expected_text.encode_utf16().count(),
                    },
                    time,
                )
                .unwrap()
                .unwrap();
            let line = code
                .lines
                .iter()
                .find(|line| line.source_id == source_id && line.occurrence == 0)
                .unwrap();
            let expected_bounds = renderer
                .measure_text_range(
                    &super::CodeLine::new(
                        "expected-overlay",
                        vec![super::StyledSpan::new(
                            expected_text,
                            super::SyntaxStyle::Plain,
                        )],
                    ),
                    expected_text,
                )
                .unwrap();
            assert_eq!(selected.bounds.x, expected_bounds.x);
            assert_eq!(selected.bounds.width, expected_bounds.width);
            assert_eq!(
                selected.line_y,
                code.timeline.sample(&line.y, time).unwrap().position
                    - code.overlays.scroll_y.sample(time).position.max(0.)
            );
            let sample = code.sample(&mut renderer, time).unwrap();
            match section {
                "promises-only-happy-path" => assert!(!sample.bright_text.is_empty()),
                "effect-sync" => assert!(sample.annotations.iter().any(|a| a.effect
                    == if step == 10 {
                        AnnotationEffect::FocusPulse
                    } else {
                        AnnotationEffect::PrismaticBloom
                    })),
                "effect-shows-errors" => assert!(!sample.squiggles.is_empty()),
                "abort-signal-infusion" => {
                    assert!(code.overlays.scroll_y.sample(time).position > 0.01)
                }
                _ => unreachable!(),
            }
            renderer.set_file_name(&format!("{chapter}/{section}.ts"));
            let pixels = code.render(&mut renderer, time).unwrap();
            code.render(&mut renderer, 0.).unwrap();
            code.render(&mut renderer, time + 0.5).unwrap();
            assert!(
                pixels == code.render(&mut renderer, time).unwrap(),
                "{section} step {step} out-of-order pixels"
            );
            eprintln!("published proof: {section} step {step} at {time:.6}s");
            if let Some(directory) = &output {
                let file =
                    std::fs::File::create(directory.join(format!("{section}-{step:02}.png")))
                        .unwrap();
                let mut encoder =
                    png::Encoder::new(std::io::BufWriter::new(file), super::WIDTH, super::HEIGHT);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .unwrap()
                    .write_image_data(&pixels)
                    .unwrap();
            }
        }
    }

    #[test]
    fn published_chapters_resolve_expected_sections() {
        let root = Path::new(super::WORKSPACE_ROOT).join("assets/effect-institute");
        let intro = PublishedChapter::load("intro", &root.join("intro")).unwrap();
        let basics = PublishedChapter::load("basics", &root.join("basics")).unwrap();

        assert_eq!(intro.sections.len(), 14);
        assert_eq!(basics.sections.len(), 16);
        assert_eq!(
            intro
                .sections
                .iter()
                .map(|section| section.sound_layers.len())
                .sum::<usize>(),
            20
        );
        assert_eq!(
            basics
                .sections
                .iter()
                .map(|section| section.sound_layers.len())
                .sum::<usize>(),
            7
        );
        assert_eq!(intro.sections[0].id, "promises-only-happy-path");
        assert_eq!(basics.sections[0].id, "effect-is-a-description");
        assert!(
            (intro.scene.duration().as_seconds() - 450.381).abs() < 0.001,
            "unexpected intro duration: {}",
            intro.scene.duration().as_seconds()
        );
        assert!(
            (basics.scene.duration().as_seconds() - 676.634).abs() < 0.001,
            "unexpected basics duration: {}",
            basics.scene.duration().as_seconds()
        );
        for chapter in [&intro, &basics] {
            let section_ranges = chapter
                .segments
                .iter()
                .filter_map(|segment| match segment {
                    super::ChapterSegment::Section { range, .. } => Some(*range),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let scripts = chapter
                .scene
                .media()
                .iter()
                .filter(|placement| placement.role() == MediaRole::Script)
                .collect::<Vec<_>>();
            let sound_count = chapter
                .sections
                .iter()
                .map(|section| section.sound_layers.len())
                .sum::<usize>();
            assert_eq!(scripts.len(), chapter.sections.len());
            assert_eq!(
                chapter.scene.media().len(),
                chapter.sections.len() + sound_count
            );
            for (placement, range) in scripts.into_iter().zip(section_ranges) {
                assert_eq!(placement.timeline_range(), range);
            }
        }
    }

    #[test]
    fn imported_frames_reconstruct_all_published_code() {
        let root = Path::new(super::WORKSPACE_ROOT).join("assets/effect-institute");
        for chapter in ["intro", "basics"] {
            let chapter = PublishedChapter::load(chapter, &root.join(chapter)).unwrap();
            for section in &chapter.sections {
                if let Some(code) = &section.code {
                    assert!(!code.frames.is_empty(), "{} has no frames", section.id);
                    assert!(!code.lines.is_empty(), "{} has no lines", section.id);
                    for frame in &code.frames {
                        for key in super::normalized_line_order(frame) {
                            let line = super::canonical_line(&code.lines, &key).unwrap();
                            super::visible_line(frame, line).unwrap();
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn duplicate_zero_time_steps_select_the_later_published_frame() {
        let code = published_code("intro", "abort-signal-infusion");

        assert_eq!(code.step_times[0], 0.0);
        assert_eq!(code.step_times[1], 0.0);
        assert_eq!(
            code.step_times
                .partition_point(|step| *step <= 0.0)
                .saturating_sub(1),
            1
        );
    }

    #[test]
    fn inline_versions_preserve_published_insertion_order() {
        let code = published_code("basics", "effect-is-a-description");
        let line = code
            .lines
            .iter()
            .find(|line| line.source_id == "def")
            .unwrap();
        let versions = line
            .variants
            .iter()
            .filter(|variant| variant.part_id == "impl:*/0")
            .filter_map(|variant| variant.version.as_deref())
            .collect::<Vec<_>>();

        assert_eq!(versions, ["Clock.currentTimeMillis", "() => Date.now()"]);
    }

    #[test]
    fn cursor_retargets_without_position_discontinuity() {
        let code = published_code("intro", "promises-only-happy-path");
        let event = code
            .overlays
            .cursor_x_columns
            .events
            .iter()
            .find(|event| event.is_spring())
            .unwrap();
        let before = code
            .overlays
            .cursor_x_columns
            .sample((event.at() - 0.0001) as f32)
            .position;
        let at = code
            .overlays
            .cursor_x_columns
            .sample(event.at() as f32)
            .position;

        assert!((before - at).abs() < 0.01);
    }

    #[test]
    fn imported_focus_state_and_long_frame_camera_survive_compilation() {
        let effect_sync = published_code("basics", "effect-sync");
        assert!(effect_sync.frames.iter().any(|frame| {
            frame.state.values().any(|line| line.dimmed) && frame.status.is_some()
        }));
        let (frame_index, line_id) = effect_sync
            .frames
            .iter()
            .enumerate()
            .find_map(|(index, frame)| {
                (effect_sync
                    .step_times
                    .get(index + 1)
                    .copied()
                    .unwrap_or(f32::MAX)
                    - effect_sync.step_times[index]
                    > 0.8)
                    .then(|| {
                        frame
                            .state
                            .iter()
                            .find(|(_, state)| state.dimmed)
                            .map(|(id, _)| (index, id.clone()))
                    })
                    .flatten()
            })
            .unwrap();
        let prepared = effect_sync
            .lines
            .iter()
            .find(|line| line.source_id == line_id)
            .unwrap();
        let settled_time = effect_sync.step_times[frame_index] + 0.75;
        assert!(
            effect_sync
                .timeline
                .sample(&prepared.blur, settled_time)
                .unwrap()
                .position
                < 0.05
        );
        assert!(
            (effect_sync
                .timeline
                .sample(&prepared.opacity, settled_time)
                .unwrap()
                .position
                - 0.4)
                .abs()
                < 0.02
        );

        let promises = published_code("intro", "promises-only-happy-path");
        let range = promises
            .frames
            .iter()
            .filter_map(|frame| frame.spotlight.as_ref())
            .flat_map(|spotlight| spotlight.ranges.values())
            .find_map(|ranges| ranges.first())
            .unwrap();
        assert!(range.start < range.end);

        let abort = published_code("intro", "abort-signal-infusion");
        for frame in abort
            .frames
            .iter()
            .filter(|frame| frame.line_order.len() > 14)
        {
            if let Some(cursor) = &frame.cursor {
                let y = cursor.line_index as f32 * super::LINE_HEIGHT - frame_scroll_target(frame);
                assert!((0.0..14.0 * super::LINE_HEIGHT).contains(&y));
            }
        }
    }

    #[test]
    fn persistent_success_status_keeps_its_original_start_time() {
        let code = published_code("basics", "effect-sync");
        for index in 1..code.frames.len() {
            let Some(status) = &code.frames[index].status else {
                continue;
            };
            for (line, ranges) in &status.ranges {
                let line_index = line.parse().unwrap();
                for range in ranges.iter().filter(|range| range.status == "success") {
                    let started = super::status_started_at(
                        &code.frames,
                        &code.step_times,
                        index,
                        line_index,
                        range,
                    );
                    if started < code.step_times[index] {
                        assert!(code.step_times[index] - started > 0.0);
                        return;
                    }
                }
            }
        }
        panic!("effect-sync has no persistent success status fixture");
    }

    #[test]
    fn component_nodes_keep_identity_and_previous_payload_during_resets() {
        let root = Path::new(super::WORKSPACE_ROOT).join("assets/effect-institute");
        let basics = PublishedChapter::load("basics", &root.join("basics")).unwrap();
        let section = basics
            .sections
            .iter()
            .find(|section| section.id == "sync-vs-succeed")
            .unwrap();
        let component = section.component.as_ref().unwrap();
        let initial = component_nodes(&component.flow.name, &component.flow.snapshots[0]);
        assert_eq!(initial.len(), 3);
        assert!(initial.iter().all(|node| node.state == "hidden"));

        let history = component.task_tracks["run2State"].sample(component.step_times[7] as f32);
        assert_eq!(
            history.previous,
            &TaskState::Succeeded(Some("09:41:00".to_owned()))
        );

        let errors = basics
            .sections
            .iter()
            .find(|section| section.id == "errors-short-circuit")
            .unwrap()
            .component
            .as_ref()
            .unwrap();
        let fail_name = component_nodes(&errors.flow.name, &errors.flow.snapshots[11]);
        let get_user = fail_name.iter().find(|node| node.key == "getUser").unwrap();
        assert_eq!(get_user.error.as_deref(), Some("NoSams"));
    }

    #[test]
    fn published_bursts_preserve_positive_negative_and_sad_polarity() {
        let burst_effects = |section_id: &str| {
            published_code("intro", section_id)
                .frames
                .iter()
                .flat_map(|frame| frame.bursts.iter())
                .map(|burst| burst.effect())
                .collect::<Vec<_>>()
        };

        assert!(burst_effects("promises-only-happy-path").contains(&AnnotationEffect::SadPulse));
        assert!(burst_effects("catch-blocks-go-stale").contains(&AnnotationEffect::DangerPulse));
        assert!(burst_effects("catch-tag").contains(&AnnotationEffect::PrismaticBloom));
    }

    #[test]
    fn canonical_lines_preserve_occurrence_identity_and_missing_state_errors() {
        let slideshow = serde_json::from_value(serde_json::json!({
            "template": {"lines": {
                "same": {"partOrder": ["body"], "parts": {"body": {"tokens": [{"text": "same()"}]}}},
                "other": {"partOrder": ["body"], "parts": {"body": {"tokens": [{"text": "other()"}]}}}
            }},
            "frames": [{
                "lineOrder": ["same", "other", "same"],
                "state": {"same": {"parts": {"body": true}}, "other": {"parts": {"body": true}}}
            }]
        })).unwrap();
        let code =
            super::PublishedCode::compile("test", "occurrences", slideshow, vec![0.]).unwrap();
        for (occurrence, y) in [(0, 0.), (1, super::LINE_HEIGHT * 2.)] {
            let line = super::canonical_line(&code.lines, &("same".into(), occurrence)).unwrap();
            assert_eq!(
                line.line.id.as_str(),
                format!("test/occurrences/same#{occurrence}")
            );
            assert_eq!(
                super::visible_line(&code.frames[0], line).unwrap(),
                "same()"
            );
            assert_eq!(code.timeline.sample(&line.y, 0.).unwrap().position, y);
            let mut missing = code.frames[0].clone();
            missing.state.remove("same");
            assert_eq!(
                super::visible_line(&missing, line).unwrap_err().to_string(),
                "visible line has no frame state"
            );
        }
        assert_eq!(
            super::canonical_line(&code.lines, &("same".into(), 2))
                .err()
                .unwrap()
                .to_string(),
            "no canonical line for 'same#2'"
        );
    }
}

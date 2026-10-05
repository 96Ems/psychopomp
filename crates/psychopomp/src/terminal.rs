//! Terminals: a window of CommitMono rows where commands are typed after a
//! prompt at a natural cadence, output prints or streams line by line, and a
//! task line spins until it resolves into a check or a cross.
//!
//! Every line has a stable ID. Its `line.<id>.reveal` channel opens its row
//! (0 to 1), and the layout is a pure function of those channels: a line sits
//! below the sum of the reveals above it, and the window shows the last
//! `rows` of the content, so once the window is full a new line opening at
//! the bottom slides every older one up by exactly its room. `scroll` is a
//! floor on the first visible row: `clear` springs it to the content height,
//! lifting everything typed so far out of the top of the window.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    author::{ActorHandle, ContinuousHandle, PlanBuilder, seconds},
    caption::CaptionSpanPlan,
    effects::spinner::{self, Mark},
    math::{easing::Ease, random::hash},
    tone::Tone,
    window,
};

pub const TERMINAL_RECIPE: &str = "terminal";

const DEFAULT_SIZE: f32 = 22.0;
/// Row pitch, in font sizes.
const ROW_EM: f32 = 1.5;
/// Content inset from the window's sides, and below the title bar.
pub const PADDING: [f32; 2] = [30.0, 20.0];
const MAX_LINES: usize = 2000;

/// A printed line opens its row this quickly; a streamed burst staggers.
const OPEN_SECONDS: f32 = 0.24;
const PRINT_GAP: f64 = 0.045;
const CLEAR_SECONDS: f32 = 0.5;
const HIGHLIGHT_IN_SECONDS: f32 = 0.18;
const HIGHLIGHT_OUT_SECONDS: f32 = 0.35;
/// The hand reaching for the keys after a prompt appears, and the beat
/// before Enter.
const REACH_SECONDS: f64 = 0.32;
const ENTER_SECONDS: f64 = 0.26;
/// A cursor blinks on for 0.53 s and off for 0.53 s while it waits.
const BLINK_SECONDS: f64 = 0.53;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalPlan {
    /// Top-left corner of the window.
    pub origin: [f32; 2],
    pub width: f32,
    /// Text rows the window shows; more scroll up through it.
    pub rows: u32,
    #[serde(default = "default_size", skip_serializing_if = "is_default_size")]
    pub size: f32,
    /// Title bar text; omitted, the bar shows only its window controls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Spans before every command, such as `~/opencode ❯ `.
    #[serde(default = "default_prompt", skip_serializing_if = "is_default_prompt")]
    pub prompt: Vec<CaptionSpanPlan>,
    /// Every line that may appear, in order. A line without a `reveal`
    /// channel is shown from time zero.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<TerminalLinePlan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TerminalLinePlan {
    /// A command after the prompt; its `typed` channel reveals it by character.
    #[serde(rename_all = "camelCase")]
    Command { id: String, text: String },
    /// Printed output; `typed` streams it by character. No spans is a blank line.
    #[serde(rename_all = "camelCase")]
    Output {
        id: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        spans: Vec<CaptionSpanPlan>,
    },
    /// A spinner leading `spans`, clocked by `spin`; `mark` draws the
    /// resolved shape and `status` cross-fades to `done`.
    #[serde(rename_all = "camelCase")]
    Task {
        id: String,
        spans: Vec<CaptionSpanPlan>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        done: Vec<CaptionSpanPlan>,
        #[serde(default, skip_serializing_if = "Mark::is_check")]
        mark: Mark,
    },
}

impl TerminalLinePlan {
    pub fn id(&self) -> &str {
        match self {
            Self::Command { id, .. } | Self::Output { id, .. } | Self::Task { id, .. } => id,
        }
    }

    /// Characters the `typed` channel reveals.
    pub fn chars(&self) -> usize {
        match self {
            Self::Command { text, .. } => text.chars().count(),
            Self::Output { spans, .. } => spans.iter().map(|s| s.text.chars().count()).sum(),
            Self::Task { .. } => 0,
        }
    }

    fn spans(&self) -> impl Iterator<Item = &CaptionSpanPlan> {
        let (first, second): (&[CaptionSpanPlan], &[CaptionSpanPlan]) = match self {
            Self::Command { .. } => (&[], &[]),
            Self::Output { spans, .. } => (spans, &[]),
            Self::Task { spans, done, .. } => (spans, done),
        };
        first.iter().chain(second)
    }

    /// Whether `channel` applies to this kind of line.
    pub fn accepts(&self, channel: TerminalChannel) -> bool {
        use TerminalChannel::*;
        match channel {
            Reveal | Highlight => true,
            Typed => !matches!(self, Self::Task { .. }),
            Spin | Mark | Status => matches!(self, Self::Task { .. }),
        }
    }
}

fn default_size() -> f32 {
    DEFAULT_SIZE
}

fn is_default_size(size: &f32) -> bool {
    *size == DEFAULT_SIZE
}

pub fn default_prompt() -> Vec<CaptionSpanPlan> {
    vec![CaptionSpanPlan::new("❯ ", Tone::Accent)]
}

fn is_default_prompt(prompt: &Vec<CaptionSpanPlan>) -> bool {
    *prompt == default_prompt()
}

/// A per-line channel: `line.<id>.<name>`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalChannel {
    /// Row presence, 0 to 1: opens the line's room and fades it in.
    Reveal,
    /// Fraction of the line's characters shown.
    Typed,
    /// An accent bar behind the row.
    Highlight,
    /// The task spinner's motor age in seconds (-1 before it starts).
    Spin,
    /// Seconds since the spinner handed off to its mark (-1 before).
    Mark,
    /// 0 shows the task's `spans`, 1 its `done` spans.
    Status,
}

impl TerminalChannel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Reveal => "reveal",
            Self::Typed => "typed",
            Self::Highlight => "highlight",
            Self::Spin => "spin",
            Self::Mark => "mark",
            Self::Status => "status",
        }
    }

    /// Split `line.<id>.<name>` into the line ID and channel.
    pub fn parse(property: &str) -> Option<(&str, Self)> {
        let (id, name) = window::split_channel(property, "line")?;
        let channel = match name {
            "reveal" => Self::Reveal,
            "typed" => Self::Typed,
            "highlight" => Self::Highlight,
            "spin" => Self::Spin,
            "mark" => Self::Mark,
            "status" => Self::Status,
            _ => return None,
        };
        Some((id, channel))
    }
}

/// The channel property for one line.
pub fn line_property(id: &str, channel: TerminalChannel) -> String {
    format!("line.{id}.{}", channel.name())
}

/// The channels a terminal takes besides its lines': the window's, plus
/// `scroll` (a floor on the first visible row) and `caret` (its presence).
pub fn accepts_property(property: &str) -> bool {
    window::is_window_property(property) || matches!(property, "scroll" | "caret")
}

/// One visible row of a sampled layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalRow {
    /// Index into `TerminalPlan::lines`.
    pub line: usize,
    /// Rows from the top of the window; negative while sliding out above it.
    pub y: f32,
    pub reveal: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerminalLayout {
    pub rows: Vec<TerminalRow>,
    /// The first visible content row.
    pub top: f32,
    /// Content height in rows.
    pub height: f32,
}

impl TerminalPlan {
    pub fn new(origin: [f32; 2], width: f32, rows: u32) -> Self {
        Self {
            origin,
            width,
            rows,
            size: DEFAULT_SIZE,
            title: None,
            prompt: default_prompt(),
            lines: Vec::new(),
        }
    }

    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn prompt(mut self, spans: Vec<CaptionSpanPlan>) -> Self {
        self.prompt = spans;
        self
    }

    pub fn row_height(&self) -> f32 {
        (self.size * ROW_EM).round()
    }

    /// The window's `[width, height]`.
    pub fn window_size(&self) -> [f32; 2] {
        [
            self.width,
            window::TITLE_BAR + PADDING[1] * 2.0 + self.rows as f32 * self.row_height(),
        ]
    }

    /// Top-left corner of the first content row.
    pub fn content_origin(&self) -> [f32; 2] {
        [
            self.origin[0] + PADDING[0],
            self.origin[1] + window::TITLE_BAR + PADDING[1],
        ]
    }

    pub fn find(&self, id: &str) -> Option<usize> {
        self.lines.iter().position(|line| line.id() == id)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.iter().all(|v| v.is_finite()),
            "terminal origin must be finite"
        );
        ensure!(
            self.width.is_finite() && (240.0..=3800.0).contains(&self.width),
            "terminal width must be between 240 and 3800"
        );
        ensure!(
            (1..=80).contains(&self.rows),
            "terminal rows must be between 1 and 80"
        );
        ensure!(
            (10.0..=72.0).contains(&self.size),
            "terminal size must be between 10 and 72"
        );
        ensure!(
            self.lines.len() <= MAX_LINES,
            "terminals are limited to {MAX_LINES} lines"
        );
        let mut ids = std::collections::HashSet::new();
        for line in &self.lines {
            ensure!(
                window::valid_id(line.id()),
                "terminal line id '{}' must be letters, digits, '-', or '_'",
                line.id()
            );
            ensure!(
                ids.insert(line.id()),
                "terminal line id '{}' is repeated",
                line.id()
            );
            if let TerminalLinePlan::Command { text, .. } = line {
                ensure!(
                    !text.contains('\n'),
                    "terminal command '{}' must be one line",
                    line.id()
                );
            }
            ensure!(
                line.spans().all(|span| !span.text.contains('\n')),
                "terminal line '{}' spans are single-line; print another line instead",
                line.id()
            );
        }
        ensure!(
            self.prompt.iter().all(|span| !span.text.contains('\n')),
            "the terminal prompt must be one line"
        );
        if let Some(title) = &self.title {
            let needed = title.chars().count() as f32 * 9.0 + 150.0;
            ensure!(
                self.width + 1e-3 >= needed,
                "terminal title '{title}' needs width >= {:.0}, got {:.0}",
                needed.ceil(),
                self.width
            );
        }
        let max_columns = ((self.width - PADDING[0] * 2.0) / (self.size * 0.6)).floor() as usize;
        let prompt_columns: usize = self.prompt.iter().map(|s| s.text.chars().count()).sum();
        for line in &self.lines {
            let columns = match line {
                TerminalLinePlan::Command { text, .. } => prompt_columns + text.chars().count() + 1,
                TerminalLinePlan::Output { spans, .. } => {
                    spans.iter().map(|s| s.text.chars().count()).sum()
                }
                TerminalLinePlan::Task { spans, done, .. } => {
                    let before: usize = spans.iter().map(|s| s.text.chars().count()).sum();
                    let after: usize = done.iter().map(|s| s.text.chars().count()).sum();
                    2 + before.max(after)
                }
            };
            ensure!(
                columns <= max_columns,
                "terminal line '{}' is {columns} columns, wider than the {max_columns}-column window",
                line.id()
            );
        }
        Ok(())
    }

    /// Every visible row given each line's reveal and the `scroll` floor.
    pub fn layout(&self, reveals: &[f32], scroll: f32) -> TerminalLayout {
        assert_eq!(reveals.len(), self.lines.len(), "one reveal per line");
        let mut rows = Vec::new();
        let mut y = 0.0;
        for (line, reveal) in reveals.iter().enumerate() {
            let reveal = reveal.clamp(0.0, 1.0);
            if reveal > 0.0 {
                rows.push(TerminalRow { line, y, reveal });
            }
            y += reveal;
        }
        let window = self.rows as f32;
        let top = scroll.max(y - window).max(0.0);
        rows.retain_mut(|row| {
            row.y -= top;
            row.y + 1.0 > 0.0 && row.y < window
        });
        TerminalLayout {
            rows,
            top,
            height: y,
        }
    }
}

/// Keystroke times for `text` typed from `at_nanos` at about
/// `chars_per_second`: deterministic, quicker inside words, with a beat
/// before each new word and after separators. One time per character.
pub fn keystrokes(text: &str, at_nanos: u64, chars_per_second: f32, seed: u32) -> Vec<u64> {
    let base = 1.0 / f64::from(chars_per_second.max(1.0));
    let mut time = 0.0;
    let mut previous = None;
    text.chars()
        .enumerate()
        .map(|(index, c)| {
            let mut factor = 0.62 + 0.62 * f64::from(hash(index as u32, seed));
            match previous {
                Some(' ') => factor *= 1.55,
                Some('-' | '/' | '.' | ':' | '=' | '|' | '"' | '\'') => factor *= 1.2,
                _ => {}
            }
            if c.is_ascii_uppercase() || "!@#$%^&*()_+{}|:\"<>?~".contains(c) {
                factor *= 1.25;
            }
            previous = Some(c);
            time += base * factor;
            at_nanos + seconds(time)
        })
        .collect()
}

/// Authoring handle for one terminal. Lines are appended as the scene is
/// authored, so every call below grows the recipe and writes its channels.
#[derive(Clone, Debug)]
pub struct TerminalActor {
    actor: ActorHandle,
    plan: TerminalPlan,
    /// Content rows once every authored reveal settles.
    height: f32,
    scroll: f32,
    /// A prompt waiting for its command: its line and when it opened.
    waiting: Option<(String, u64)>,
    tasks: Vec<(String, u64)>,
    caret: Option<ContinuousHandle>,
}

impl TerminalActor {
    /// Lines already in `plan` are shown from time zero.
    pub fn declare(
        scene: &mut PlanBuilder,
        id: impl Into<String>,
        plan: TerminalPlan,
    ) -> Result<Self> {
        plan.validate()?;
        let actor = scene.actor(id, TERMINAL_RECIPE, &plan)?;
        Ok(Self {
            actor,
            height: plan.lines.len() as f32,
            plan,
            scroll: 0.0,
            waiting: None,
            tasks: Vec::new(),
            caret: None,
        })
    }

    pub fn actor(&self) -> &ActorHandle {
        &self.actor
    }

    pub fn id(&self) -> &str {
        self.actor.id()
    }

    pub fn plan(&self) -> &TerminalPlan {
        &self.plan
    }

    pub fn channel(
        &mut self,
        scene: &mut PlanBuilder,
        property: &str,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, property, initial)
    }

    fn line_channel(
        &mut self,
        scene: &mut PlanBuilder,
        id: &str,
        channel: TerminalChannel,
        initial: f32,
    ) -> ContinuousHandle {
        scene.channel(&self.actor, &line_property(id, channel), initial)
    }

    /// Settle the window in; its content follows. Returns when it has landed.
    pub fn show(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> u64 {
        window::settle_in(scene, &self.actor, at_nanos)
    }

    pub fn hide(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        window::dismiss(scene, &self.actor, at_nanos);
    }

    fn caret(&mut self, scene: &mut PlanBuilder) -> ContinuousHandle {
        if self.caret.is_none() {
            self.caret = Some(self.channel(scene, "caret", 0.0));
        }
        self.caret.clone().expect("declared above")
    }

    /// Append a line and open its row at `at_nanos`.
    fn push(
        &mut self,
        scene: &mut PlanBuilder,
        line: TerminalLinePlan,
        at_nanos: u64,
    ) -> Result<String> {
        let id = line.id().to_owned();
        ensure!(
            self.plan.find(&id).is_none(),
            "terminal line id '{id}' is repeated"
        );
        self.plan.lines.push(line);
        self.plan.validate()?;
        scene.replace_actor_data(&self.actor, &self.plan)?;
        let reveal = self.line_channel(scene, &id, TerminalChannel::Reveal, 0.0);
        scene.spring(&reveal, at_nanos, 1.0, OPEN_SECONDS, 0.0);
        self.height += 1.0;
        Ok(id)
    }

    fn next_id(&self, prefix: &str) -> String {
        format!("{prefix}{}", self.plan.lines.len())
    }

    fn replace_line(&mut self, scene: &mut PlanBuilder, line: TerminalLinePlan) -> Result<()> {
        let index = self
            .plan
            .find(line.id())
            .with_context(|| format!("terminal line '{}' is not declared", line.id()))?;
        self.plan.lines[index] = line;
        self.plan.validate()?;
        scene.replace_actor_data(&self.actor, &self.plan)?;
        Ok(())
    }

    /// Blink the caret, which is on at `from`, until `until`, where it is on.
    fn blink(&mut self, scene: &mut PlanBuilder, from: u64, until: u64) {
        let caret = self.caret(scene);
        let half = seconds(BLINK_SECONDS);
        let mut at = from + half;
        let mut lit = true;
        while at + half / 2 < until {
            lit = !lit;
            scene.ease(&caret, at, f32::from(u8::from(lit)), 0.08, Ease::Smoothstep);
            at += half;
        }
        if !lit {
            scene.ease(&caret, until, 1.0, 0.05, Ease::Linear);
        }
    }

    /// Open a fresh prompt at `at_nanos`; its caret blinks until the next
    /// [`Self::type_command`] types into it. Returns the line ID.
    pub fn prompt(&mut self, scene: &mut PlanBuilder, at_nanos: u64) -> Result<String> {
        ensure!(
            self.waiting.is_none(),
            "a terminal prompt is already waiting for its command"
        );
        let id = self.next_id("cmd");
        self.push(
            scene,
            TerminalLinePlan::Command {
                id: id.clone(),
                text: String::new(),
            },
            at_nanos,
        )?;
        let caret = self.caret(scene);
        scene.ease(&caret, at_nanos, 1.0, 0.06, Ease::Linear);
        self.waiting = Some((id.clone(), at_nanos));
        Ok(id)
    }

    /// Let a waiting prompt's caret blink until `until_nanos`, as a terminal
    /// idles at the end of a scene.
    pub fn idle(&mut self, scene: &mut PlanBuilder, until_nanos: u64) {
        if let Some((id, since)) = self.waiting.take() {
            self.blink(scene, since, until_nanos);
            self.waiting = Some((id, until_nanos.max(since)));
        }
    }

    /// Type `text` at about 18 characters per second. See [`Self::type_at`].
    pub fn type_command(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        text: &str,
    ) -> Result<u64> {
        self.type_at(scene, at_nanos, text, 18.0)
    }

    /// Type `text` into a waiting prompt, or open one at `at_nanos` and type
    /// after a short reach. The caret is solid while typing and leaves on
    /// Enter, which this returns: a beat after the last keystroke.
    pub fn type_at(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        text: &str,
        chars_per_second: f32,
    ) -> Result<u64> {
        let (id, start) = match self.waiting.take() {
            Some((id, since)) => {
                let start = at_nanos.max(since);
                self.blink(scene, since, start);
                (id, start)
            }
            None => {
                let id = self.prompt(scene, at_nanos)?;
                self.waiting = None;
                (id, at_nanos + seconds(REACH_SECONDS))
            }
        };
        self.replace_line(
            scene,
            TerminalLinePlan::Command {
                id: id.clone(),
                text: text.to_owned(),
            },
        )?;
        let typed = self.line_channel(scene, &id, TerminalChannel::Typed, 0.0);
        let chars = text.chars().count();
        let seed = self.plan.lines.len() as u32;
        let times = keystrokes(text, start, chars_per_second, seed);
        for (index, at) in times.iter().enumerate() {
            scene.set(&typed, *at, (index + 1) as f32 / chars as f32);
        }
        let enter = times.last().copied().unwrap_or(start) + seconds(ENTER_SECONDS);
        let caret = self.caret(scene);
        scene.set(&caret, enter, 0.0);
        Ok(enter)
    }

    /// Print output lines from `at_nanos`, each a moment after the last, as
    /// a command streams them. Returns when the last line opens.
    pub fn print(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        lines: impl IntoIterator<Item = Vec<CaptionSpanPlan>>,
    ) -> Result<u64> {
        let mut at = at_nanos;
        let mut last = at_nanos;
        for spans in lines {
            let id = self.next_id("out");
            self.push(scene, TerminalLinePlan::Output { id, spans }, at)?;
            last = at;
            at += seconds(PRINT_GAP);
        }
        Ok(last)
    }

    /// Print one plain or toned line per `\n`-separated line of `text`.
    pub fn print_text(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        text: &str,
        tone: Tone,
    ) -> Result<u64> {
        let lines = text
            .lines()
            .map(|line| {
                if line.is_empty() {
                    Vec::new()
                } else {
                    vec![CaptionSpanPlan::new(line, tone)]
                }
            })
            .collect::<Vec<_>>();
        self.print(scene, at_nanos, lines)
    }

    /// One output line whose characters stream in at `chars_per_second`,
    /// as an agent's reply does. Returns when the last character appears.
    pub fn stream(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        spans: Vec<CaptionSpanPlan>,
        chars_per_second: f32,
    ) -> Result<u64> {
        let id = self.next_id("out");
        let line = TerminalLinePlan::Output { id, spans };
        let chars = line.chars();
        let typed = scene.channel(
            &self.actor,
            &line_property(line.id(), TerminalChannel::Typed),
            0.0,
        );
        self.push(scene, line, at_nanos)?;
        Ok(crate::caption::type_steps(
            scene,
            &typed,
            at_nanos,
            chars.max(1),
            chars_per_second,
        ))
    }

    /// A task line led by a spinner that starts at `at_nanos`. Returns its ID
    /// for [`Self::resolve`].
    pub fn spin(
        &mut self,
        scene: &mut PlanBuilder,
        at_nanos: u64,
        spans: Vec<CaptionSpanPlan>,
    ) -> Result<String> {
        let id = self.next_id("task");
        self.push(
            scene,
            TerminalLinePlan::Task {
                id: id.clone(),
                spans,
                done: Vec::new(),
                mark: Mark::Check,
            },
            at_nanos,
        )?;
        let spin = self.line_channel(scene, &id, TerminalChannel::Spin, -1.0);
        clock(scene, &spin, at_nanos);
        self.tasks.push((id.clone(), at_nanos));
        Ok(id)
    }

    /// Resolve a spinning task into `mark` at the motor's next top-right
    /// crossing after `at_nanos`, cross-fading its text to `done` (when not
    /// empty). Returns when the mark has finished drawing.
    pub fn resolve(
        &mut self,
        scene: &mut PlanBuilder,
        task: &str,
        at_nanos: u64,
        mark: Mark,
        done: Vec<CaptionSpanPlan>,
    ) -> Result<u64> {
        let started = self
            .tasks
            .iter()
            .find(|(id, _)| id == task)
            .map(|(_, at)| *at)
            .with_context(|| format!("terminal task '{task}' is not spinning"))?;
        let Some(TerminalLinePlan::Task { spans, .. }) =
            self.plan.find(task).map(|index| &self.plan.lines[index])
        else {
            bail!("terminal line '{task}' is not a task");
        };
        let spans = spans.clone();
        let has_done = !done.is_empty();
        self.replace_line(
            scene,
            TerminalLinePlan::Task {
                id: task.to_owned(),
                spans,
                done,
                mark,
            },
        )?;
        let waited = (at_nanos.saturating_sub(started)) as f32 / 1e9;
        let age = spinner::handoff(waited);
        let handoff = started + seconds(f64::from(age));
        // Once the mark has drawn and cooled the pose is still, so both
        // clocks stop and later samples merge.
        let rest = spinner::DRAW + spinner::COOL;
        let mark = self.line_channel(scene, task, TerminalChannel::Mark, -1.0);
        scene.set(&mark, handoff, 0.0);
        scene.ease(&mark, handoff, rest, rest, Ease::Linear);
        let spin = self.line_channel(scene, task, TerminalChannel::Spin, -1.0);
        let still = handoff + seconds(f64::from(rest));
        scene.set(&spin, still, age + rest);
        let drawn = handoff + seconds(f64::from(spinner::DRAW));
        if has_done {
            let status = self.line_channel(scene, task, TerminalChannel::Status, 0.0);
            scene.spring(
                &status,
                handoff + seconds(f64::from(spinner::DRAW) * 0.5),
                1.0,
                0.4,
                0.0,
            );
        }
        Ok(drawn)
    }

    /// Light a line's highlight bar at `at_nanos` and let it go `seconds` later.
    pub fn highlight(
        &mut self,
        scene: &mut PlanBuilder,
        line: &str,
        at_nanos: u64,
        for_seconds: f32,
    ) -> Result<()> {
        ensure!(
            self.plan.find(line).is_some(),
            "terminal line '{line}' is not declared"
        );
        let channel = self.line_channel(scene, line, TerminalChannel::Highlight, 0.0);
        scene.spring(&channel, at_nanos, 1.0, HIGHLIGHT_IN_SECONDS, 0.0);
        let off = at_nanos + seconds(f64::from(for_seconds.max(0.0)));
        scene.spring(&channel, off, 0.0, HIGHLIGHT_OUT_SECONDS, 0.0);
        Ok(())
    }

    /// Lift everything printed so far out of the top of the window, as
    /// `clear` does; the next line starts at the top.
    pub fn clear(&mut self, scene: &mut PlanBuilder, at_nanos: u64) {
        let height = self.height;
        self.scroll_to(scene, height, at_nanos);
    }

    /// Spring the first visible row to at least `row`.
    pub fn scroll_to(&mut self, scene: &mut PlanBuilder, row: f32, at_nanos: u64) {
        let channel = self.channel(scene, "scroll", 0.0);
        scene.spring(&channel, at_nanos, row, CLEAR_SECONDS, 0.0);
        self.scroll = row;
    }
}

/// Start an elapsed-seconds clock at `at_nanos` that runs to the scene's end.
fn clock(scene: &mut PlanBuilder, channel: &ContinuousHandle, at_nanos: u64) {
    let seconds = (scene.duration_nanos().saturating_sub(at_nanos) / 1_000_000) as f32 / 1000.0;
    scene.set(channel, at_nanos, 0.0);
    if seconds > 0.0 {
        scene.ease(channel, at_nanos, seconds, seconds, Ease::Linear);
    }
}

#[cfg(test)]
mod tests;

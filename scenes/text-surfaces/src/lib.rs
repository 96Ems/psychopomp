//! Text-surface showroom: an agent session in a terminal, a Slack thread
//! that reacts to it, and the pull request's changed files, each drawn
//! natively so it follows the theme and re-times with the scene.
use anyhow::Result;
use psychopomp::{
    author::{PlanBuilder, SECOND},
    caption::CaptionSpanPlan,
    changed_files::{ChangedFilePlan, ChangedFilesActor, ChangedFilesPlan, FileStatus},
    chat::{ChatActor, ChatPersonPlan, ChatPlan, ChatSpanPlan, ChatStyle},
    effects::spinner::Mark,
    plan::{ReelPlan, ScenePlan},
    terminal::{TerminalActor, TerminalPlan},
    tone::Tone,
};

const MS: u64 = 1_000_000;

pub fn build_reel() -> Result<ReelPlan> {
    ReelPlan::dipped(
        "text-surfaces",
        vec![
            build_terminal()?,
            build_slack()?,
            build_messages()?,
            build_changed_files()?,
        ],
        600 * MS,
    )
}

fn span(text: &str, tone: Tone) -> CaptionSpanPlan {
    CaptionSpanPlan::new(text, tone)
}

fn plain(text: &str) -> ChatSpanPlan {
    ChatSpanPlan::plain(text)
}

/// The team's channel reacts: a flaky test is reported, someone types, the
/// agent streams its fix, and the thread celebrates.
pub fn build_slack() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("slack", 15 * SECOND);
    let mut chat = ChatActor::declare(
        &mut scene,
        "thread",
        ChatPlan::new(
            [460.0, 110.0],
            [1000.0, 860.0],
            vec![
                ChatPersonPlan::new("adam", "Adam Elmore", Tone::Request),
                ChatPersonPlan::new("dax", "Dax Raad", Tone::Warning),
                ChatPersonPlan::new("bot", "opencode", Tone::Accent).badge("APP"),
            ],
        )
        .titled("# opencode-dev", Some("CI, flakes, and releases"))
        .composer("Message #opencode-dev"),
    )?;
    let shown = chat.show(&mut scene, 200 * MS);
    chat.stamp("10:41 AM");
    let first = chat.say(
        &mut scene,
        shown,
        "adam",
        vec![
            plain("anyone else seeing "),
            ChatSpanPlan::code("session.test.ts"),
            plain(" flake on main?"),
        ],
    )?;
    chat.say_text(
        &mut scene,
        shown + 1100 * MS,
        "adam",
        "failed twice in the last hour",
    )?;
    chat.stamp("10:42 AM");
    chat.typing(&mut scene, shown + 2000 * MS, "dax")?;
    chat.say_text(
        &mut scene,
        shown + 3600 * MS,
        "dax",
        "yeah, it races compaction. asking the agent",
    )?;
    chat.react(&mut scene, shown + 4300 * MS, &first, "👀", 2)?;
    chat.stamp("10:44 AM");
    chat.typing(&mut scene, shown + 5000 * MS, "bot")?;
    let (fix, streamed) = chat.stream(
        &mut scene,
        shown + 6800 * MS,
        "bot",
        vec![
            plain("Fixed in #51842: the test now awaits "),
            ChatSpanPlan::code("compaction.settled()"),
            plain(" instead of sleeping. 24 pass, 0 fail."),
        ],
        55.0,
    )?;
    chat.react(&mut scene, streamed + 400 * MS, &fix, "🎉", 3)?;
    chat.react(&mut scene, streamed + 650 * MS, &fix, "🚀", 1)?;
    chat.highlight(&mut scene, &fix, streamed + 300 * MS, 2.2)?;
    chat.typing(&mut scene, streamed + 1200 * MS, "adam")?;
    chat.say_text(&mut scene, streamed + 2300 * MS, "adam", "ship it")?;
    Ok(scene.finish()?)
}

/// The same component in its bubbles style: a short text exchange.
pub fn build_messages() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("messages", 9 * SECOND);
    let mut chat = ChatActor::declare(
        &mut scene,
        "messages",
        ChatPlan::new(
            [610.0, 120.0],
            [700.0, 840.0],
            vec![
                ChatPersonPlan::new("kit", "Kit", Tone::Request),
                ChatPersonPlan::new("olive", "Olive", Tone::Success),
            ],
        )
        .style(ChatStyle::Bubbles)
        .me("kit")
        .titled("Olive", Some("iMessage"))
        .composer("iMessage"),
    )?;
    let shown = chat.show(&mut scene, 200 * MS);
    chat.say_text(&mut scene, shown, "olive", "did the release go out?")?;
    chat.typing(&mut scene, shown + 900 * MS, "kit")?;
    chat.say_text(
        &mut scene,
        shown + 2300 * MS,
        "kit",
        "yep, the flaky test is fixed too",
    )?;
    chat.say_text(&mut scene, shown + 3000 * MS, "kit", "the agent found it")?;
    chat.typing(&mut scene, shown + 3800 * MS, "olive")?;
    let reply = chat.say_text(
        &mut scene,
        shown + 5600 * MS,
        "olive",
        "nice!! dinner at 7?",
    )?;
    chat.react(&mut scene, shown + 6600 * MS, &reply, "😂", 1)?;
    Ok(scene.finish()?)
}

/// An agent fixes a flaky test: the command is typed, the agent's progress
/// streams in, the test run spins and passes, and the fix is committed and
/// pushed, scrolling the session up through the window, then cleared.
pub fn build_terminal() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("terminal", 20 * SECOND);
    let mut term = TerminalActor::declare(
        &mut scene,
        "term",
        TerminalPlan::new([300.0, 170.0], 1320.0, 15)
            .size(23.0)
            .titled("kit@studio — ~/code/opencode — zsh")
            .prompt(vec![
                span("~/code/opencode", Tone::Request),
                span(" ❯ ", Tone::Success),
            ]),
    )?;
    let shown = term.show(&mut scene, 200 * MS);
    let entered = term.type_command(
        &mut scene,
        shown,
        "opencode run \"fix the flaky session test\"",
    )?;
    let tool = |verb: &str, path: &str| {
        vec![
            span("● ", Tone::Accent),
            span(verb, Tone::Plain),
            span(path, Tone::Muted),
        ]
    };
    let mut at = term.print(
        &mut scene,
        entered + 250 * MS,
        [
            vec![],
            tool("Read ", "packages/opencode/src/session/compaction.ts"),
            tool("Read ", "packages/opencode/test/session.test.ts"),
        ],
    )?;
    at = term.stream(
        &mut scene,
        at + 500 * MS,
        vec![span(
            "  The test asserts before compaction settles; awaiting it.",
            Tone::Plain,
        )],
        70.0,
    )?;
    at = term.print(
        &mut scene,
        at + 350 * MS,
        [
            tool("Edit ", "packages/opencode/test/session.test.ts"),
            vec![span("    + await compaction.settled()", Tone::Success)],
            vec![span("    - await sleep(50)", Tone::Error)],
        ],
    )?;
    let task = term.spin(
        &mut scene,
        at + 450 * MS,
        vec![span("Running bun test test/session.test.ts", Tone::Muted)],
    )?;
    let passed = term.resolve(
        &mut scene,
        &task,
        at + 2450 * MS,
        Mark::Check,
        vec![
            span("24 pass", Tone::Success),
            span("  0 fail  ", Tone::Muted),
            span("[412ms]", Tone::Muted),
        ],
    )?;
    term.highlight(&mut scene, &task, passed + 200 * MS, 1.6)?;
    at = term.print(
        &mut scene,
        passed + 300 * MS,
        [
            vec![],
            vec![span(
                "Done. The flaky assertion now waits for compaction.",
                Tone::Plain,
            )],
            vec![],
        ],
    )?;
    term.prompt(&mut scene, at + 300 * MS)?;
    let entered = term.type_command(
        &mut scene,
        at + 1700 * MS,
        "git commit -am \"fix(session): await compaction\" && git push",
    )?;
    at = term.print(
        &mut scene,
        entered + 200 * MS,
        [
            vec![span(
                "[dev 3f2a91c] fix(session): await compaction",
                Tone::Plain,
            )],
            vec![
                span(" 1 file changed, ", Tone::Muted),
                span("1 insertion(+)", Tone::Success),
                span(", ", Tone::Muted),
                span("1 deletion(-)", Tone::Error),
            ],
        ],
    )?;
    at = term.print(
        &mut scene,
        at + 700 * MS,
        [
            vec![span("To github.com:sst/opencode.git", Tone::Muted)],
            vec![span("   9c1e2d4..3f2a91c  dev -> dev", Tone::Muted)],
            vec![],
        ],
    )?;
    term.prompt(&mut scene, at + 300 * MS)?;
    let cleared = term.type_command(&mut scene, at + 1600 * MS, "clear")?;
    term.clear(&mut scene, cleared);
    term.prompt(&mut scene, cleared + 150 * MS)?;
    term.idle(&mut scene, 20 * SECOND);
    Ok(scene.finish()?)
}

/// The pull request's opener: its changed files cascade in with their
/// diffstats while the totals roll, then one file takes focus.
pub fn build_changed_files() -> Result<ScenePlan> {
    let mut scene = PlanBuilder::new("changed-files", 9 * SECOND);
    let file = |id: &str, path: &str, status, added, removed| {
        ChangedFilePlan::new(
            id,
            format!("packages/opencode/{path}"),
            status,
            added,
            removed,
        )
    };
    let mut card = ChangedFilesActor::declare(
        &mut scene,
        "files",
        ChangedFilesPlan::new(
            [310.0, 190.0],
            1300.0,
            vec![
                file(
                    "compaction",
                    "src/session/compaction.ts",
                    FileStatus::Modified,
                    38,
                    11,
                ),
                file(
                    "settled",
                    "src/session/settled.ts",
                    FileStatus::Added,
                    54,
                    0,
                ),
                file("test", "test/session.test.ts", FileStatus::Modified, 6, 2),
                file(
                    "fixture",
                    "test/fixtures/compaction.json",
                    FileStatus::Added,
                    212,
                    0,
                ),
                file("sleep", "src/util/sleep.ts", FileStatus::Deleted, 0, 17),
                file("wait", "src/util/wait.ts", FileStatus::Modified, 3, 3)
                    .renamed_from("packages/opencode/src/util/delay.ts"),
                ChangedFilePlan::new(
                    "changeset",
                    ".changeset/quiet-owls-wait.md",
                    FileStatus::Added,
                    5,
                    0,
                ),
            ],
        )
        .titled("opencode #51842 · fix(session): await compaction before asserting"),
    )?;
    let shown = card.show(&mut scene, 200 * MS);
    let landed = card.reveal(&mut scene, shown + 150 * MS, 0.11)?;
    card.focus(&mut scene, "compaction", landed + 900 * MS)?;
    card.focus(&mut scene, "test", landed + 2600 * MS)?;
    card.unfocus(&mut scene, landed + 4200 * MS);
    Ok(scene.finish()?)
}

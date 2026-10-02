//! A narrated walkthrough of five OpenCode background-service pull requests.
//!
//! Each PR gets two segments: a sequence diagram that plays the broken behavior,
//! then replays the fixed behavior in the same slots, and an editor that animates
//! the actual change as a diff. Every visual moment is keyed to a phrase in the
//! narration, so re-voicing the script re-times the film.
pub mod film;
mod flagship;

pub use flagship::build_flagship;

use std::path::Path;

use anyhow::{Context, Result};
use film::{
    Flow, HEADER_Y, LEFT, Pr, TRANSITION, after, after_following, before, behavior, code, span,
};
use psychopomp::{
    author::{PlanBuilder, seconds},
    caption::{CaptionActor, CaptionAlign, CaptionPlan},
    editor::diff::{Diff, add, keep, remove},
    narration::Narration,
    plan::{ReelPlan, ScenePlan},
    sequence::{
        SequenceActor, SequenceParticipantPlan as Participant, SequencePlan, SequenceRowPlan as Row,
    },
    tone::Tone,
};

const PRS: [Pr; 5] = [
    Pr {
        number: "#50784",
        title: "keep the real startup error",
        slug: "errors",
    },
    Pr {
        number: "#50825",
        title: "never kill a healthy server",
        slug: "mismatch",
    },
    Pr {
        number: "#50782",
        title: "retry the bind during restarts",
        slug: "bind",
    },
    Pr {
        number: "#50042",
        title: "stop waits for the process",
        slug: "stop",
    },
    Pr {
        number: "#50840",
        title: "the TUI reconnects during startup",
        slug: "bootstrap",
    },
];

pub fn build_reel(narration_dir: &Path) -> Result<ReelPlan> {
    let narration = Narration::load(narration_dir)?;
    let mut plans = vec![intro(&narration)?];
    for (index, pr) in PRS.iter().enumerate() {
        plans.push(behavior(pr, &narration, flows(index))?);
        plans.push(code(pr, &narration, diffs(index), true)?);
    }
    plans.push(outro(&narration)?);
    ReelPlan::dipped("pr-walkthrough", plans, TRANSITION)
}

fn intro(narration: &Narration) -> Result<ScenePlan> {
    let clip = narration.clip("intro")?;
    let lead = seconds(0.8);
    let duration = lead + clip.duration() + seconds(2.2);
    let mut scene = PlanBuilder::new("intro", duration);
    let spoken = clip.place(&mut scene, lead);

    let title = CaptionPlan::line(
        [LEFT, HEADER_Y],
        30.0,
        vec![
            span("opencode", Tone::Accent),
            span("  background service", Tone::Plain),
        ],
    );
    CaptionActor::declare(&mut scene, "header", &title)?.type_in(
        &mut scene,
        seconds(0.25),
        50.0,
        0.8,
    );

    let flow = SequencePlan {
        origin: [250.0, 190.0],
        width: 1480.0,
        row_height: 84.0,
        slots: Some(6),
        participants: vec![
            Participant::new("tui", "TUI", "terminal 1"),
            Participant::new("desktop", "desktop app", ""),
            Participant::new("service", "opencode service", "one per machine"),
            Participant::new("tui2", "TUI", "terminal 2"),
        ],
        rows: vec![
            Row::message("tui", "tui", "service", "connect", Tone::Request),
            Row::message("desktop", "desktop", "service", "connect", Tone::Request),
            Row::message("tui2", "tui2", "service", "connect", Tone::Request),
            Row::note(
                "timeout",
                &["tui", "tui2"],
                "failed to start → \"Timed out waiting…\"",
                Tone::Error,
            )
            .in_slot(4),
            Row::end("killed", "service", "a healthy server, killed", Tone::Error).in_slot(5),
        ],
    };
    let mut sequence = SequenceActor::declare(&mut scene, "flow", &flow)?;
    sequence.animate(&mut scene, "opacity", 0.0, seconds(0.3), 1.0, 0.5);
    sequence.animate(&mut scene, "lifelines", 0.0, seconds(0.5), 1.0, 1.0);
    let connect = spoken.at("connect to it");
    sequence.reveal(&mut scene, "tui", spoken.at("every terminal"));
    sequence.reveal(&mut scene, "desktop", spoken.at("the desktop app"));
    sequence.reveal(&mut scene, "tui2", connect);
    sequence.reveal(&mut scene, "timeout", spoken.at("vague timeout"));
    sequence.reveal(&mut scene, "killed", spoken.at("got killed"));
    let list_at = spoken.at("these five small pull requests");
    sequence.animate(
        &mut scene,
        "opacity",
        0.0,
        list_at - seconds(0.3),
        0.0,
        0.45,
    );

    // One caret at a time: each row starts typing when the previous one lands.
    let mut cursor = list_at + seconds(0.25);
    for (index, pr) in PRS.iter().enumerate() {
        let plan = CaptionPlan::line(
            [560.0, 330.0 + index as f32 * 84.0],
            34.0,
            vec![
                span(pr.number, Tone::Accent),
                span("   ", Tone::Plain),
                span(pr.title, Tone::Plain),
            ],
        );
        let mut row = CaptionActor::declare(&mut scene, format!("pr-{}", pr.slug), &plan)?;
        cursor = row.type_in(&mut scene, cursor, 80.0, 0.0) + seconds(0.12);
    }
    scene.finish().context("intro")
}

fn outro(narration: &Narration) -> Result<ScenePlan> {
    let clip = narration.clip("outro")?;
    let lead = seconds(0.7);
    let duration = lead + clip.duration() + seconds(2.6);
    let mut scene = PlanBuilder::new("outro", duration);
    let spoken = clip.place(&mut scene, lead);
    let title = CaptionPlan::line(
        [LEFT, HEADER_Y],
        30.0,
        vec![
            span("merge order", Tone::Accent),
            span("  small · tested · independent", Tone::Muted),
        ],
    );
    CaptionActor::declare(&mut scene, "header", &title)?.show(&mut scene, seconds(0.1));
    let first_two = spoken.at("start with the first two");
    let the_rest = spoken.at("then bind retries");
    for (index, pr) in PRS.iter().enumerate() {
        let plan = CaptionPlan::line(
            [470.0, 330.0 + index as f32 * 84.0],
            34.0,
            vec![
                span(&format!("{}.", index + 1), Tone::Muted),
                span("  ", Tone::Plain),
                span(pr.number, Tone::Accent),
                span("   ", Tone::Plain),
                span(pr.title, Tone::Plain),
            ],
        );
        let mut row = CaptionActor::declare(&mut scene, format!("pr-{}", pr.slug), &plan)?;
        let opacity = row.channel(&mut scene, "opacity", 0.0);
        scene.spring(&opacity, seconds(0.3 + index as f64 * 0.12), 0.35, 0.5, 0.0);
        let highlight = if index < 2 { first_two } else { the_rest };
        scene.spring(
            &opacity,
            highlight + seconds(index as f64 * 0.25),
            1.0,
            0.4,
            0.0,
        );
        scene.spring(&opacity, duration - seconds(1.2), 0.0, 0.6, 0.0);
    }
    let mut closing = CaptionActor::declare(
        &mut scene,
        "closing",
        &CaptionPlan::line(
            [960.0, 820.0],
            30.0,
            vec![
                span("each one is ", Tone::Plain),
                span("small, tested, and independent", Tone::Accent),
            ],
        )
        .aligned(CaptionAlign::Center),
    )?;
    closing.type_in(&mut scene, spoken.at("each one is small"), 40.0, 1.2);
    closing.hide(&mut scene, duration - seconds(1.2));
    scene.finish().context("outro")
}

// ---------------------------------------------------------------------------
// The five behavior stories
// ---------------------------------------------------------------------------

fn flows(index: usize) -> Flow {
    match index {
        0 => errors_flow(),
        1 => mismatch_flow(),
        2 => bind_flow(),
        3 => stop_flow(),
        _ => bootstrap_flow(),
    }
}

fn errors_flow() -> Flow {
    let sequence = SequencePlan {
        origin: [250.0, 168.0],
        width: 1480.0,
        row_height: 80.0,
        slots: Some(8),
        participants: vec![
            Participant::new("client", "client", "Service.ensure"),
            Participant::new("a", "contender A", ""),
            Participant::new("b", "contender B", ""),
            Participant::new("c", "contender C", ""),
        ],
        rows: vec![
            Row::message("spawn-a", "client", "a", "spawn", Tone::Request).with_aside("0 s"),
            Row::message("spawn-b", "client", "b", "spawn", Tone::Request).with_aside("5 s"),
            Row::reply(
                "fail-a",
                "a",
                "client",
                "error: port 49374 in use",
                Tone::Error,
            ),
            Row::end("end-a", "a", "exits", Tone::Muted),
            Row::message(
                "drop",
                "client",
                "client",
                "B is alive → drop it",
                Tone::Warning,
            ),
            Row::message(
                "spawn-c",
                "client",
                "c",
                "spawn a replacement",
                Tone::Request,
            )
            .with_aside("10 s"),
            Row::note(
                "repeat",
                &["a", "c"],
                "…again, and every failure is dropped",
                Tone::Muted,
            ),
            Row::note(
                "timeout",
                &["client", "c"],
                "Timed out waiting for the background service to start",
                Tone::Error,
            )
            .with_aside("120 s"),
            Row::message(
                "keep",
                "client",
                "client",
                "keep the first error",
                Tone::Success,
            )
            .in_slot(4),
            Row::note("no-new", &["b", "c"], "no new contenders", Tone::Success).in_slot(5),
            Row::reply("b-exits", "b", "client", "exits too", Tone::Muted).in_slot(6),
            Row::note(
                "real",
                &["client", "c"],
                "reported: port 49374 is already in use",
                Tone::Error,
            )
            .in_slot(7),
        ],
    };
    Flow {
        sequence,
        before_only: &["drop", "spawn-c", "repeat", "timeout"],
        reveals: vec![
            ("spawn-a", before("starts contenders")),
            ("spawn-b", before("each try to become")),
            ("fail-a", before("fails with")),
            ("end-a", before("real useful error")),
            ("drop", before("only reports a failure")),
            ("spawn-c", before("keeps starting new ones")),
            ("repeat", before("over and over")),
            ("timeout", before("timed out waiting")),
            ("keep", after("keeps the first error")),
            ("no-new", after("stops starting replacements")),
            ("b-exits", after("if none of them does")),
            ("real", after("the actual cause")),
        ],
        strikes: vec![("fail-a", before("dropped"))],
        emphasis: vec![("a", before("fails with"), before("only reports a failure"))],
        late: vec![("c", before("keeps starting new ones").plus(-0.2), 0.25)],
        footer_before: (
            vec![
                span("a real error becomes a ", Tone::Plain),
                span("two-minute timeout", Tone::Error),
            ],
            before("two minutes later"),
        ),
        footer_after: (
            vec![
                span("the ", Tone::Plain),
                span("first error", Tone::Success),
                span(" is the one you see", Tone::Plain),
            ],
            after("the actual cause"),
        ),
    }
}

fn mismatch_flow() -> Flow {
    let sequence = SequencePlan {
        origin: [300.0, 168.0],
        width: 1320.0,
        row_height: 96.0,
        slots: Some(6),
        participants: vec![
            Participant::new("client", "client", "reconnecting"),
            Participant::new("service", "opencode service", "healthy"),
            Participant::new("others", "other clients", ""),
        ],
        rows: vec![
            Row::message("probe", "client", "service", "GET /api/info", Tone::Request),
            Row::reply("missing", "service", "client", "404", Tone::Error),
            Row::message(
                "assume",
                "client",
                "client",
                "404 → outdated → replace it",
                Tone::Warning,
            ),
            Row::message("kill", "client", "service", "SIGTERM", Tone::Error),
            Row::end("stopped", "service", "stopped", Tone::Error),
            Row::note(
                "cut",
                &["service", "others"],
                "every other client disconnects",
                Tone::Error,
            ),
            Row::message(
                "decide",
                "client",
                "client",
                "version ok → protocol mismatch",
                Tone::Success,
            )
            .in_slot(2),
            Row::note(
                "refuse",
                &["client"],
                "error: update this client, or restart explicitly",
                Tone::Warning,
            )
            .in_slot(3),
            Row::note(
                "alive",
                &["service", "others"],
                "the server keeps running",
                Tone::Success,
            )
            .in_slot(4),
        ],
    };
    Flow {
        sequence,
        before_only: &["assume", "kill", "stopped", "cut"],
        reveals: vec![
            ("probe", before("health endpoint")),
            ("missing", before("returns a 404")),
            ("assume", before("assumed the server was outdated")),
            ("kill", before("then start a new one").plus(-1.0)),
            ("stopped", before("then start a new one")),
            ("cut", before("cut off every other client")),
            ("decide", after("health protocols")),
            ("refuse", after("clear message")),
            ("alive", after("nothing gets killed")),
        ],
        strikes: vec![],
        emphasis: vec![(
            "service",
            before("then start a new one").plus(-1.0),
            before("cut off every other client"),
        )],
        late: vec![],
        footer_before: (
            vec![
                span("a 404 meant: ", Tone::Plain),
                span("kill the server", Tone::Error),
            ],
            before("cut off every other client"),
        ),
        footer_after: (
            vec![
                span("a mismatch is an ", Tone::Plain),
                span("error", Tone::Warning),
                span(", never a kill", Tone::Plain),
            ],
            after("nothing gets killed"),
        ),
    }
}

fn bind_flow() -> Flow {
    let sequence = SequencePlan {
        origin: [300.0, 168.0],
        width: 1320.0,
        row_height: 88.0,
        slots: Some(7),
        participants: vec![
            Participant::new("new", "new server", "starting"),
            Participant::new("port", "port 49374", ""),
            Participant::new("old", "old server", "exiting"),
        ],
        rows: vec![
            Row::message("bind", "new", "port", "bind", Tone::Request),
            Row::reply("in-use", "port", "new", "EADDRINUSE", Tone::Error),
            Row::message(
                "wait",
                "new",
                "new",
                "wait for an owner to register",
                Tone::Warning,
            )
            .with_aside("0 s"),
            Row::message("release", "old", "port", "release", Tone::Muted),
            Row::end("exited", "old", "exited", Tone::Muted),
            Row::message("still", "new", "new", "still waiting…", Tone::Warning).with_aside("5 s"),
            Row::note(
                "fail",
                &["new", "port"],
                "port 49374 is in use by another process",
                Tone::Error,
            )
            .with_aside("15 s"),
            Row::message(
                "retry",
                "new",
                "new",
                "retry the bind every 100 ms",
                Tone::Success,
            )
            .in_slot(2),
            Row::message("bind-again", "new", "port", "bind", Tone::Success).in_slot(5),
            Row::reply("listening", "port", "new", "listening", Tone::Success).in_slot(6),
        ],
    };
    Flow {
        sequence,
        before_only: &["wait", "still", "fail"],
        reveals: vec![
            ("bind", before("bind the port")),
            ("in-use", before("address in use")),
            ("wait", before("waits up to fifteen seconds")),
            ("release", before("isn't coming back")),
            ("exited", before("it's exiting")),
            ("still", before("keeps waiting")),
            ("fail", before("then fails")),
            ("retry", after("retries the bind")),
            ("bind-again", after("as soon as the port is free")),
            ("listening", after("starts normally")),
        ],
        strikes: vec![],
        emphasis: vec![("port", before("frees up"), before("keeps waiting"))],
        late: vec![],
        footer_before: (
            vec![
                span("the port frees up, but it ", Tone::Plain),
                span("never tries again", Tone::Error),
            ],
            before("then fails"),
        ),
        footer_after: (
            vec![
                span("same process, ", Tone::Plain),
                span("port acquired", Tone::Success),
            ],
            after("starts normally"),
        ),
    }
}

fn stop_flow() -> Flow {
    let sequence = SequencePlan {
        origin: [250.0, 168.0],
        width: 1480.0,
        row_height: 82.0,
        slots: Some(8),
        participants: vec![
            Participant::new("client", "client", "restart"),
            Participant::new("old", "old server", "pid 4127"),
            Participant::new("file", "service.json", ""),
            Participant::new("new", "new server", ""),
        ],
        rows: vec![
            Row::message("term", "client", "old", "SIGTERM", Tone::Error),
            Row::message("grace", "client", "client", "wait up to 5 s", Tone::Plain)
                .with_aside("0 s"),
            Row::message("unregister", "old", "file", "unregister", Tone::Muted),
            Row::message("check", "client", "file", "still ours?", Tone::Request).with_aside("5 s"),
            Row::reply("gone", "file", "client", "gone", Tone::Muted),
            Row::message(
                "assume",
                "client",
                "client",
                "assume it stopped",
                Tone::Warning,
            ),
            Row::message("start", "client", "new", "start", Tone::Request),
            Row::note(
                "clash",
                &["old", "new"],
                "the old process still holds the port",
                Tone::Error,
            ),
            Row::message(
                "watch",
                "client",
                "client",
                "watch pid 4127, not the file",
                Tone::Success,
            )
            .in_slot(3)
            .with_aside("5 s"),
            Row::message("kill", "client", "old", "SIGKILL", Tone::Error).in_slot(4),
            Row::end("exited", "old", "exited", Tone::Muted).in_slot(5),
            Row::message("start-clean", "client", "new", "start", Tone::Success).in_slot(6),
            Row::note("clean", &["new"], "the port is free", Tone::Success).in_slot(7),
        ],
    };
    Flow {
        sequence,
        before_only: &["check", "gone", "assume", "start", "clash"],
        reveals: vec![
            ("term", before("the client sends")),
            ("grace", before("waits for the server to exit")),
            ("unregister", before("registration file")),
            ("check", before("disappeared or changed")),
            ("gone", before("disappeared or changed").plus(0.6)),
            ("assume", before("gave up without escalating")),
            ("start", before("restart that followed")),
            ("clash", before("collide with it")),
            ("watch", after("only trusts the process")),
            ("kill", after("it gets")),
            ("exited", after("it gets").plus(0.8)),
            ("start-clean", after("if even that fails").plus(-0.6)),
            ("clean", after("if even that fails")),
        ],
        strikes: vec![],
        emphasis: vec![(
            "old",
            before("still running"),
            before("restart that followed"),
        )],
        late: vec![],
        footer_before: (
            vec![
                span("trusted the file, so the restart ", Tone::Plain),
                span("collided", Tone::Error),
            ],
            before("collide with it"),
        ),
        footer_after: (
            vec![
                span("SIGKILL if needed · ", Tone::Plain),
                span("a clear error", Tone::Warning),
                span(" if even that fails", Tone::Plain),
            ],
            after_following("clear error", "if even that fails"),
        ),
    }
}

fn bootstrap_flow() -> Flow {
    let sequence = SequencePlan {
        origin: [250.0, 168.0],
        width: 1480.0,
        row_height: 76.0,
        slots: Some(9),
        participants: vec![
            Participant::new("tui", "TUI", "startup"),
            Participant::new("a", "server A", ""),
            Participant::new("ensure", "Service.ensure", ""),
            Participant::new("b", "server B", ""),
        ],
        rows: vec![
            Row::message("probe", "tui", "a", "GET /api/info", Tone::Request),
            Row::reply("ready", "a", "tui", "200 · ready", Tone::Success),
            Row::end("gone", "a", "goes away", Tone::Error),
            Row::message("list", "tui", "a", "file.list", Tone::Request),
            Row::note(
                "crash",
                &["tui", "a"],
                "ClientError: Transport → the TUI exits",
                Tone::Error,
            ),
            Row::message(
                "reconnect",
                "tui",
                "ensure",
                "reconnect, once",
                Tone::Accent,
            )
            .in_slot(4),
            Row::message("start", "ensure", "b", "start", Tone::Request).in_slot(5),
            Row::reply("endpoint", "ensure", "tui", "endpoint B", Tone::Success).in_slot(6),
            Row::message("retry", "tui", "b", "file.list", Tone::Request).in_slot(7),
            Row::reply("home", "b", "tui", "location → home screen", Tone::Success).in_slot(8),
        ],
    };
    Flow {
        sequence,
        before_only: &["crash"],
        reveals: vec![
            ("probe", before("finds a healthy server")),
            ("ready", before("finds a healthy server").plus(0.7)),
            ("gone", before("goes away in between")),
            ("list", before("that one request fails")),
            ("crash", before("transport error")),
            ("reconnect", after("one managed reconnect")),
            ("start", after("find or start a server")),
            ("endpoint", after("rebuild the client")),
            ("retry", after("try again")),
            ("home", after("two minute deadline")),
        ],
        strikes: vec![],
        emphasis: vec![(
            "a",
            before("goes away in between"),
            before("transport error"),
        )],
        late: vec![("b", after("find or start a server"), 1.0)],
        footer_before: (
            vec![
                span("one lost server → ", Tone::Plain),
                span("the TUI exits", Tone::Error),
            ],
            before("transport error"),
        ),
        footer_after: (
            vec![
                span("one reconnect, then ", Tone::Plain),
                span("home", Tone::Success),
                span(" · explicit servers never start one", Tone::Muted),
            ],
            after("explicit server connection"),
        ),
    }
}

// ---------------------------------------------------------------------------
// The five changes, condensed for display
// ---------------------------------------------------------------------------

fn diffs(index: usize) -> (Diff, Vec<&'static str>, &'static str) {
    match index {
        0 => (
            Diff {
                file_name: "client/effect/service.ts",
                lines: vec![
                    add(1, "let failure: Error | undefined"),
                    keep("// … each poll:"),
                    remove(1, "const failure = finished"),
                    add(1, "failure ??= finished"),
                    keep("  .map(contenderFailure)"),
                    keep("  .find((error): error is Error => error !== undefined)"),
                    keep("if (failure !== undefined && contenders.size === 0)"),
                    keep("  return yield* Effect.fail(failure)"),
                    remove(
                        2,
                        "if (contenders.size < 2 && Date.now() - lastSpawn >= spawnDelay) {",
                    ),
                    add(2, "if (failure === undefined && contenders.size < 2 &&"),
                    add(2, "    Date.now() - lastSpawn >= spawnDelay) {"),
                    keep("  contenders.add(yield* spawnContender)"),
                    keep("}"),
                    keep("// … after the deadline:"),
                    remove(3, "return yield* Effect.fail("),
                    add(3, "return yield* Effect.fail(failure ??"),
                    keep("  new Error(\"Timed out waiting for the background service to start\"))"),
                ],
            },
            vec![
                "lives outside the loop",
                "no new contender",
                "the timeout reports",
            ],
            "condensed for display · the promise client gets the same change",
        ),
        1 => (
            Diff {
                file_name: "client/effect/service.ts",
                lines: vec![
                    keep("if (service !== undefined) {"),
                    remove(1, "  const compatible ="),
                    remove(
                        1,
                        "    service.compatible && matchesVersion(service.version, options)",
                    ),
                    add(
                        1,
                        "  const versionMatches = matchesVersion(service.version, options)",
                    ),
                    add(
                        1,
                        "  const compatible = service.compatible && versionMatches",
                    ),
                    add(2, "  if (!service.compatible && versionMatches)"),
                    add(2, "    return yield* Effect.fail(new Error("),
                    add(
                        2,
                        "      \"Background service uses an incompatible health protocol. \" +",
                    ),
                    add(
                        2,
                        "        \"Update this client or explicitly restart the service.\"))",
                    ),
                    keep("  if (compatible && service.state === \"ready\") return service"),
                    keep("  // … otherwise:"),
                    keep("  yield* announce(\"version-mismatch\", service.version)"),
                    keep("  yield* stop({ file: options.file })"),
                    keep("}"),
                ],
            },
            vec![
                "checks the version requirement first",
                "if the version is fine",
            ],
            "condensed for display · the promise client gets the same change",
        ),
        2 => (
            Diff {
                file_name: "cli/server-process.ts",
                lines: vec![
                    remove(1, "const server = yield* start(options, lifecycle)"),
                    remove(1, "  .pipe(Effect.catch((error) => {"),
                    add(1, "const launch = start(options, lifecycle)"),
                    add(
                        1,
                        "const server = yield* launch.pipe(Effect.catch((error) => {",
                    ),
                    keep("  if (!addressInUse(error)) return Effect.fail(error)"),
                    remove(
                        1,
                        "  // wait up to 15 s for an owner to register; never rebind",
                    ),
                    remove(
                        1,
                        "  return recognizeIncumbent(url).pipe(Effect.flatMap(orPortInUse))",
                    ),
                    add(1, "  return Effect.gen(function* () {"),
                    add(1, "    const deadline = Date.now() + 15_000"),
                    add(1, "    while (Date.now() < deadline) {"),
                    add(1, "      if (yield* incumbentAt(url)) return"),
                    add(1, "      yield* Effect.sleep(\"100 millis\")"),
                    add(
                        1,
                        "      const retry = yield* launch.pipe(ignoreAddressInUse)",
                    ),
                    add(1, "      if (retry !== undefined) return retry"),
                    add(1, "    }"),
                    add(1, "    return yield* Effect.fail(portInUse)"),
                    add(1, "  })"),
                    keep("}))"),
                ],
            },
            vec!["second wait becomes"],
            "condensed for display",
        ),
        3 => (
            Diff {
                file_name: "client/effect/service.ts",
                lines: vec![
                    keep("yield* signal(info.pid, \"SIGTERM\")"),
                    keep("const done = yield* stopped(info.pid)"),
                    keep("  .pipe(Effect.retry(poll(timing)), Effect.option)"),
                    add(
                        1,
                        "// The registration can disappear or change hands before this process",
                    ),
                    add(
                        1,
                        "// exits. Only the PID we signalled can tell us whether it has stopped.",
                    ),
                    keep("if (Option.isNone(done)) {"),
                    remove(1, "  const latest = yield* read(options.file)"),
                    remove(
                        1,
                        "  if (latest === undefined || !same(latest, info)) return",
                    ),
                    keep("  yield* signal(info.pid, \"SIGKILL\")"),
                    keep("  yield* stopped(info.pid).pipe(Effect.retry(poll(timing)))"),
                    keep("}"),
                ],
            },
            vec!["deletes two lines"],
            "condensed for display · the promise client gets the same change",
        ),
        _ => (
            Diff {
                file_name: "app.tsx + bootstrap.ts",
                lines: vec![
                    remove(1, "const api = OpenCode.make(options)"),
                    remove(1, "const location = yield* Effect.tryPromise(() =>"),
                    remove(1, "  api.file.list({ location: { directory } }),"),
                    remove(
                        1,
                        ").pipe(Effect.catch(() => Effect.tryPromise(() => api.location.get())))",
                    ),
                    add(
                        1,
                        "const connection = yield* bootstrap(input.server, process.cwd())",
                    ),
                    add(2, ""),
                    add(
                        2,
                        "const bootstrap = Effect.fn(function* (server, directory) {",
                    ),
                    add(
                        2,
                        "  return yield* connect(server.endpoint, directory).pipe(",
                    ),
                    add(2, "    Effect.catch((error) => {"),
                    add(
                        2,
                        "      if (!server.service || !isTransport(error)) return Effect.fail(error)",
                    ),
                    add(
                        2,
                        "      // one recovery: a second transport failure cannot retry again",
                    ),
                    add(2, "      return reconnect(server.service).pipe("),
                    add(
                        2,
                        "        Effect.flatMap((endpoint) => connect(endpoint, directory)))",
                    ),
                    add(2, "    }),"),
                    add(
                        2,
                        "    Effect.timeoutOrElse({ duration: \"2 minutes\", orElse: timedOut }),",
                    ),
                    add(2, "  )"),
                    add(2, "})"),
                ],
            },
            vec!["one new bootstrap function", "connect if that fails"],
            "condensed for display",
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::build_reel;

    #[test]
    fn reel_matches_the_committed_plan() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let reel = build_reel(&root.join("narration")).unwrap();
        let committed = std::fs::read_to_string(root.join("pr-walkthrough.reel.json")).unwrap();
        assert_eq!(
            serde_json::to_string_pretty(&reel).unwrap() + "\n",
            committed
        );
        assert_eq!(reel.segments.len(), 12);
        let flagship = super::build_flagship(&root.join("narration")).unwrap();
        let committed = std::fs::read_to_string(root.join("pr-50825.reel.json")).unwrap();
        assert_eq!(
            serde_json::to_string_pretty(&flagship).unwrap() + "\n",
            committed
        );
    }
}

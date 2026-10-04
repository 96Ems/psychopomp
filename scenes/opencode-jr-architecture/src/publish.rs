//! Back to Slack: a follower folds OpenCode's durable log into a turn;
//! publication edits one post per assistant message, under a working
//! indicator, and a write-ahead intent plus Slack metadata keep a crash from
//! posting twice.
use anyhow::{Context, Result};
use psychopomp::{
    author::PlanTime, caption::CaptionAlign, plan::ScenePlan, stage::StagePlan, tone::Tone,
};

use crate::{
    Film, GLITCH, IMPACT, MARK, Narration, SUCCESS, arrive, beam, begin, card, chip, footer,
    header, label, orb, orb_in, packet, plug, post, seconds, send, show, sound, status,
};

const RUNTIME: [f32; 3] = [260.0, 510.0, 0.0];
const FOLLOWER: [f32; 3] = [660.0, 510.0, 0.0];
const PUBLICATION: [f32; 3] = [1070.0, 510.0, 0.0];
const SQLITE: [f32; 3] = [1070.0, 820.0, 0.0];
const THREAD_X: f32 = 1570.0;

fn stage() -> StagePlan {
    let slack = |id: &str, y: f32, h: f32, title: &str, statuses: &[(&str, Tone)]| {
        card(
            id,
            [THREAD_X, y, 0.0],
            [360.0, h],
            title,
            statuses,
            Tone::Plain,
        )
    };
    StagePlan {
        post: post(),
        elements: vec![
            orb("runtime", RUNTIME, 80.0, 500),
            label(
                "log-name",
                [RUNTIME[0], 625.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[("durable event log", Tone::Muted)],
            ),
            card(
                "follower",
                FOLLOWER,
                [260.0, 100.0],
                "follower",
                &[
                    ("reading the log", Tone::Muted),
                    ("folding a turn", Tone::Plain),
                ],
                Tone::Plain,
            ),
            card(
                "publication",
                PUBLICATION,
                [290.0, 100.0],
                "publication",
                &[
                    ("reconciling", Tone::Muted),
                    ("intent flushed", Tone::Plain),
                    ("reset", Tone::Error),
                    ("adopted, not reposted", Tone::Success),
                ],
                Tone::Plain,
            ),
            card(
                "sqlite",
                SQLITE,
                [290.0, 90.0],
                "sqlite",
                &[
                    ("handles · cursor", Tone::Muted),
                    ("write-ahead intent", Tone::Accent),
                ],
                Tone::Plain,
            ),
            label(
                "thread-name",
                [THREAD_X, 260.0, 0.0],
                20.0,
                CaptionAlign::Center,
                &[("slack thread", Tone::Muted)],
            ),
            slack(
                "indicator",
                340.0,
                84.0,
                "working indicator",
                &[("working", Tone::Muted), ("done", Tone::Success)],
            ),
            slack(
                "answer",
                500.0,
                130.0,
                "answer",
                &[
                    ("one post per assistant message", Tone::Muted),
                    ("streaming ·", Tone::Plain),
                    ("streaming · ·", Tone::Plain),
                    ("streaming · · ·", Tone::Plain),
                    ("edited in place", Tone::Success),
                ],
            ),
            slack(
                "footer",
                675.0,
                84.0,
                "footer",
                &[
                    ("substantial or failed turns", Tone::Muted),
                    ("posted once", Tone::Success),
                ],
            ),
            label(
                "metadata",
                [THREAD_X, 768.0, 0.0],
                19.0,
                CaptionAlign::Center,
                &[
                    ("metadata: ", Tone::Muted),
                    ("its durable id", Tone::Accent),
                ],
            ),
            beam("lg", "runtime", "follower", Tone::Plain),
            beam("fp", "follower", "publication", Tone::Plain),
            beam("pi", "publication", "indicator", Tone::Plain),
            beam("pa", "publication", "answer", Tone::Request),
            beam("pf", "publication", "footer", Tone::Plain),
            beam("pw", "publication", "sqlite", Tone::Plain),
            packet("events", "lg", false, "events", Tone::Request),
            packet("turn", "fp", false, "turn", Tone::Request),
            packet("indicate", "pi", false, "", Tone::Plain),
            packet("edit-1", "pa", false, "post", Tone::Request),
            packet("edit-2", "pa", false, "edit", Tone::Request),
            packet("edit-3", "pa", false, "edit", Tone::Request),
            packet("intent", "pw", false, "intent", Tone::Accent),
            packet("post-footer", "pf", false, "post", Tone::Request),
            packet("probe", "pf", false, "find by metadata", Tone::Plain),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "publish", 1.0, 2.0, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    header(sc, "4", "back to slack")?;
    chip(sc, "src/session/publication.ts")?;

    orb_in(s, sc, "runtime", seconds(0.3));
    s.settle_in(sc, "follower", seconds(0.5));
    s.settle_in(sc, "publication", seconds(0.66));
    plug(s, sc, "lg", seconds(0.9));
    plug(s, sc, "fp", seconds(1.06));
    show(s, sc, "thread-name", seconds(1.0));
    s.to(sc, "camera.x", seconds(0.6), -120.0, 1.8);

    let log = v.at("durable event log");
    s.type_in(sc, "log-name", log, 40.0);
    let read = send(s, sc, "events", log + seconds(0.2), 0.7);
    s.land(sc, "follower", read);
    let fold = v.at("folds it into");
    status(s, sc, "follower", fold, 1);
    let folded = send(s, sc, "turn", fold.not_before(read + seconds(0.4)), 0.7);
    s.land(sc, "publication", folded);
    // The working indicator goes up first and stays above the answers.
    arrive(s, sc, "indicator", "pi", folded);
    s.clock(sc, "indicator.spinner", folded + seconds(0.5));
    s.send(sc, "indicate", folded + seconds(0.4), 0.5);
    s.to(sc, "camera.x", folded, 80.0, 1.8);

    // One Slack post per assistant message, edited in place as it streams.
    let post = v.at("one slack post");
    let contact = arrive(s, sc, "answer", "pa", post - seconds(0.5));
    let mut at = send(s, sc, "edit-1", contact + seconds(0.2), 0.6);
    for (index, edit) in ["edit-2", "edit-3"].into_iter().enumerate() {
        s.land(sc, "answer", at);
        status(s, sc, "answer", at, index + 1);
        at = send(s, sc, edit, at + seconds(0.35), 0.55);
    }
    s.land(sc, "answer", at);
    status(s, sc, "answer", at, 3);
    let edited = v.at("edited in").not_before(at + seconds(0.3));
    status(s, sc, "answer", edited, 4);
    s.hit(sc, "indicator.flash", v.at("working indicator"), 0.5, 0.0);
    let footer_at = v.at("footer only");
    s.settle_in(sc, "footer", footer_at);
    s.to(sc, "footer.dim", footer_at, 0.5, 0.4);

    // The write-ahead intent is flushed before the post can escape.
    let posting = v.at("before posting");
    s.to(sc, "camera.x", posting - seconds(0.3), 60.0, 1.6);
    s.to(sc, "camera.y", posting - seconds(0.3), 40.0, 1.6);
    let contact = arrive(s, sc, "sqlite", "pw", posting - seconds(0.9));
    let flushed = send(
        s,
        sc,
        "intent",
        (posting + seconds(0.2)).not_before(contact + seconds(0.2)),
        0.6,
    );
    s.land(sc, "sqlite", flushed);
    status(s, sc, "sqlite", flushed, 1);
    status(s, sc, "publication", flushed, 1);
    sc.media(sound("flushed", MARK, flushed, -17.0));
    plug(s, sc, "pf", flushed);
    let posted = send(s, sc, "post-footer", flushed + seconds(0.75), 0.6);
    s.land(sc, "footer", posted);
    s.to(sc, "footer.dim", posted, 0.0, 0.4);
    s.type_in(
        sc,
        "metadata",
        v.at("slack metadata").not_before(posted),
        40.0,
    );

    // A crash rolls the handle back; the post is found by its metadata.
    let crash = v.at("after a crash");
    s.hit(sc, "post.chroma", crash, 0.12, 0.0);
    s.jolt(sc, crash, [0.0, 1.0], 0.5);
    status(s, sc, "publication", crash, 2);
    for (step, seed) in [7.0, 9.0, 8.0, 0.0].into_iter().enumerate() {
        s.set(
            sc,
            "publication.glitch",
            crash + seconds(step as f64 * 0.027),
            seed,
        );
    }
    sc.media(sound("crash", IMPACT, crash, -12.0));
    sc.media(sound("crash-glitch", GLITCH, crash, -18.0));
    let adopt = v.at("adopts it");
    let found = send(s, sc, "probe", crash + seconds(0.6), 0.8);
    s.hit(sc, "footer.flash", found, 0.6, 0.0);
    status(s, sc, "footer", found, 1);
    status(s, sc, "publication", adopt.not_before(found), 3);
    s.clock(sc, "indicator.mark", adopt.not_before(found));
    status(s, sc, "indicator", adopt.not_before(found), 1);
    sc.media(sound("adopted", SUCCESS, adopt.not_before(found), -13.0));
    footer(
        sc,
        "footer-adopt",
        &[
            ("after a crash, ", Tone::Plain),
            ("adopt", Tone::Accent),
            (" the post instead of reposting", Tone::Plain),
        ],
        adopt.not_before(found),
    )?;
    scene.finish().context("publish")
}

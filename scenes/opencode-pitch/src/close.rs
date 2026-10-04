//! The line to remember, and the ask.
use anyhow::{Context, Result};
use psychopomp::{math::easing::Ease, plan::ScenePlan, stage::StagePlan, tone::Tone};

use crate::{
    BLOOM, Film, Narration, REST_Z, begin, label, orb, orb_in, post, ring, seconds, show, sound,
};

const CORE: [f32; 3] = [960.0, 380.0, 0.0];

fn stage() -> StagePlan {
    StagePlan {
        post: post(),
        elements: vec![
            orb("core", CORE, 64.0, 620),
            ring("shield", CORE, 104.0, 2.5, Tone::Accent),
            label(
                "spend",
                [960.0, 600.0, 0.0],
                54.0,
                &[("spend tokens once.", Tone::Plain)],
            ),
            label(
                "keep",
                [960.0, 680.0, 0.0],
                54.0,
                &[("keep the protection.", Tone::Accent)],
            ),
            label(
                "ask",
                [960.0, 820.0, 0.0],
                22.0,
                &[
                    ("two-week pilot", Tone::Plain),
                    ("  ·  ", Tone::Muted),
                    ("one owner", Tone::Plain),
                    ("  ·  ", Tone::Muted),
                    ("fixed budget", Tone::Plain),
                ],
            ),
        ],
    }
}

pub fn film(narration: &Narration) -> Result<ScenePlan> {
    let Film {
        sc: mut scene,
        s: mut actor,
        v,
    } = begin(narration, "close", 0.9, 3.4, &stage())?;
    let (s, sc) = (&mut actor, &mut scene);
    orb_in(s, sc, "core", seconds(0.2));
    let spend = v.at("spend tokens once");
    s.type_in(sc, "spend", spend, 26.0);
    let keep = v.at("keep the protection");
    s.type_in(sc, "keep", keep, 28.0);
    show(s, sc, "shield", keep);
    s.ease(sc, "shield.sweep", keep, 1.0, 1.1, Ease::Smootherstep);
    let closed = keep + seconds(1.1);
    s.hit(sc, "core.pulse", closed, 0.8, 0.0);
    sc.media(sound("closed", BLOOM, closed, -15.0));
    show(s, sc, "ask", v.end() + seconds(0.3));
    s.to(sc, "camera.z", v.end(), REST_Z + 50.0, 3.0);
    scene.finish().context("close")
}

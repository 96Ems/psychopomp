//! THROWAWAY: three article-embedded session/browser compositions, not a terminal
//! emulator, browser recording, live client, or new supported Scene Plan recipe.
use anyhow::{Context, Result};
use cosmic_text::{Attrs, Color, Family, FontSystem, Metrics, SwashCache, Weight};
use kinograph::{
    author::PlanBuilder,
    plan::{ScenePlan, SpringPlan, compile_channels},
    timeline::{PropertyId, Timeline},
};
use std::{collections::HashMap, fs, path::Path};

#[path = "../../../crates/kinograph-render/src/encode.rs"]
mod encode;
mod paint;
#[path = "../../../crates/kinograph-render/src/render/text/raster.rs"]
mod raster;
use paint::ui::{
    Bounds,
    card::{Clip, ContentFit, Fill, RgbaSource, SurfaceStyle, UiCanvas, UiColor},
};

const SIZE: [u32; 2] = [1920, 1080];
const FPS: u32 = 60;
const DURATION: f64 = 26.;
const TIMES: [f64; 7] = [0., 3., 6., 9., 13., 17., 21.];
const TITLES: [&str; 7] = [
    "Ask for a browser",
    "Edit configuration",
    "Tools arrive",
    "Search apartments",
    "Inspect a listing",
    "Change to Code Mode",
    "Continue in a new browser context",
];
const INK: [u8; 3] = [230, 229, 226];
const MUTED: [u8; 3] = [139, 140, 145];
const GREEN: [u8; 3] = [167, 198, 148];
const VIOLET: [u8; 3] = [184, 160, 225];
const PAPER: [u8; 3] = [235, 233, 224];
const BLACK: [u8; 3] = [41, 44, 43];

#[derive(Clone, Copy)]
enum Variant {
    Paired,
    Focus,
    Bridge,
}
impl Variant {
    fn id(self) -> &'static str {
        match self {
            Self::Paired => "A",
            Self::Focus => "B",
            Self::Bridge => "C",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Paired => "Paired workspaces",
            Self::Focus => "Focus handoff",
            Self::Bridge => "Tool bridge",
        }
    }
}

fn plan(variant: Variant) -> Result<ScenePlan> {
    let mut p = PlanBuilder::new(
        format!("session-prototype-{}", variant.id()),
        (DURATION * 1e9) as u64,
    );
    for (i, at) in TIMES.into_iter().enumerate() {
        p.presentation_step(
            format!("beat-{i}"),
            TITLES[i],
            (at * 1e9) as u64,
            ((at + 2.5) * 1e9) as u64,
        );
    }
    let (session, browser) = match variant {
        Variant::Paired => ([64., 152., 860., 788.], [996., 152., 860., 788.]),
        Variant::Focus => ([140., 152., 1160., 788.], [1370., 152., 486., 788.]),
        Variant::Bridge => ([120., 100., 1080., 350.], [650., 592., 1150., 404.]),
    };
    for (id, bounds) in [("session", session), ("browser", browser)] {
        let actor = p.actor(
            id,
            "session-story-prototype",
            serde_json::json!({"fixture":true}),
        )?;
        for (i, property) in ["x", "y", "width", "height"].into_iter().enumerate() {
            let mut values = [bounds[i]; 7];
            if matches!(variant, Variant::Focus) {
                let active = if id == "session" {
                    [64., 152., 690., 788.]
                } else {
                    [810., 152., 1046., 788.]
                };
                values[3..].fill(active[i]);
            }
            p.step_track(&actor, property, &values, SpringPlan::visual(0.55, 0.));
        }
        p.step_track(&actor, "opacity", &[1.; 7], SpringPlan::visual(0.16, 0.));
        if id == "session" {
            let scroll = p.continuous(&actor, "scroll", 0.);
            p.spring(&scroll, 17_000_000_000, 220., 0.5, 0.);
        }
    }
    Ok(p.finish()?)
}

struct Text {
    fonts: FontSystem,
    swash: SwashCache,
    sprites: HashMap<(String, u32, [u8; 3], bool, bool), raster::TextSprite>,
}
impl Text {
    fn new() -> Result<Self> {
        let mut fonts = FontSystem::new();
        let path = std::env::var("KINOGRAPH_FONT").unwrap_or_else(|_| {
            format!(
                "{}/Library/Fonts/CommitMono-400-Regular.otf",
                std::env::var("HOME").unwrap()
            )
        });
        fonts
            .db_mut()
            .load_font_file(path)
            .context("load the comparison font")?;
        Ok(Self {
            fonts,
            swash: SwashCache::new(),
            sprites: HashMap::new(),
        })
    }
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        c: &mut UiCanvas<'_>,
        pos: [f32; 2],
        text: &str,
        size: f32,
        color: [u8; 3],
        sans: bool,
        bold: bool,
        opacity: f32,
    ) {
        if text.is_empty() || opacity <= 0. {
            return;
        }
        let key = (text.to_owned(), size.to_bits(), color, sans, bold);
        let sprite = self.sprites.entry(key).or_insert_with(|| {
            let attrs = Attrs::new()
                .family(if sans {
                    Family::Name("Helvetica Neue")
                } else {
                    Family::Name("CommitMono")
                })
                .weight(if bold { Weight::BOLD } else { Weight::NORMAL })
                .color(Color::rgb(color[0], color[1], color[2]));
            let mut s = raster::make_sprite(
                &mut self.fonts,
                &mut self.swash,
                vec![(text, attrs.clone())],
                attrs,
                Metrics::new(size, size * 1.4),
                1700,
                (size * 1.6).ceil() as u32,
            );
            let w = (s.advance.ceil() as u32 + 3).clamp(1, s.width);
            s.pixels = s
                .pixels
                .chunks_exact(s.width as usize * 4)
                .flat_map(|row| row[..w as usize * 4].iter().copied())
                .collect();
            s.width = w;
            s
        });
        c.rgba(
            Bounds {
                origin: pos,
                size: [sprite.width as f32, sprite.height as f32],
            },
            RgbaSource::packed(&sprite.pixels, [sprite.width, sprite.height]).unwrap(),
            ContentFit::Fill,
            opacity,
        );
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds {
    Bounds {
        origin: [x, y],
        size: [w, h],
    }
}
fn fill(c: &mut UiCanvas<'_>, b: Bounds, radius: f32, color: [u8; 3], alpha: f32) {
    c.fill(
        b,
        radius,
        Fill::Solid(UiColor::srgb8(color[0], color[1], color[2], 255)),
        alpha,
    );
}
fn rule(c: &mut UiCanvas<'_>, x: f32, y: f32, w: f32, color: [u8; 3]) {
    fill(c, rect(x, y, w, 1.), 0., color, 1.);
}
fn fade(t: f64, at: f64) -> f32 {
    ((t - at) / 0.16).clamp(0., 1.) as f32
}
fn typed(line: &str, t: f64, at: f64, duration: f64) -> &str {
    let chars = ((t - at) / duration).clamp(0., 1.) * line.chars().count() as f64;
    let end = line
        .char_indices()
        .nth(chars as usize)
        .map_or(line.len(), |(i, _)| i);
    &line[..end]
}
fn bounds(timeline: &Timeline, id: &str, t: f64) -> Bounds {
    let v = |name: &str| {
        timeline
            .sample_at(&PropertyId::new(format!("{id}.{name}")), t)
            .unwrap()
            .position
    };
    rect(v("x"), v("y"), v("width"), v("height"))
}

fn session(
    c: &mut UiCanvas<'_>,
    text: &mut Text,
    b: Bounds,
    t: f64,
    compact: bool,
    scroll: f32,
) -> Result<()> {
    let [x, y] = b.origin;
    let [w, h] = b.size;
    c.surface(
        b,
        SurfaceStyle::new(Fill::Solid(UiColor::srgb8(17, 18, 21, 255)), 16.).border(
            1.,
            UiColor::srgb8(63, 64, 69, 255),
            1.,
        ),
        1.,
    );
    text.draw(c, [x + 30., y + 18.], "SF move", 29., INK, true, true, 1.);
    text.draw(
        c,
        [x + 166., y + 22.],
        "/ opencode",
        23.,
        MUTED,
        false,
        false,
        1.,
    );
    text.draw(
        c,
        [x + w - 180., y + 22.],
        "session 01",
        21.,
        GREEN,
        false,
        false,
        1.,
    );
    rule(c, x + 1., y + 70., w - 2., [45, 46, 50]);
    let body = rect(x + 28., y + 92., w - 56., h - 184.);
    c.clipped(Clip::rounded(body, 0.), |c| {
        if compact {
            let (label, first, second) = match t {
                t if t < 3. => (
                    "YOU",
                    "Connect Chrome DevTools.",
                    "Let’s find an apartment near the Panhandle.",
                ),
                t if t < 6. => (
                    "OPENCODE",
                    "I’ll add it to the project configuration.",
                    "Edit  .opencode/opencode.jsonc",
                ),
                t if t < 9. => (
                    "OPENCODE",
                    "New browser tools. Same session.",
                    "Chrome DevTools MCP is connected.",
                ),
                t if t < 13. => (
                    "OPENCODE",
                    "Three apartments near the Panhandle.",
                    "I’ll inspect the Oak Street listing.",
                ),
                t if t < 17. => (
                    "OPENCODE",
                    "Sunny one-bedroom. Tall windows.",
                    "The monthly rent is $48,000.",
                ),
                t if t < 21. => (
                    "OPENCODE",
                    "Code Mode enabled. Instructions updated.",
                    "The old browser context was replaced.",
                ),
                _ => (
                    "OPENCODE",
                    "The browser reopened. Our session stayed.",
                    "Still $48,000. Better make half a million.",
                ),
            };
            let at = TIMES.into_iter().rfind(|at| *at <= t).unwrap();
            text.draw(c, [x + 32., y + 94.], label, 19., VIOLET, false, false, 1.);
            text.draw(
                c,
                [x + 32., y + 129.],
                first,
                29.,
                INK,
                true,
                false,
                fade(t, at),
            );
            text.draw(
                c,
                [x + 32., y + 175.],
                second,
                25.,
                MUTED,
                true,
                false,
                fade(t, at + 0.15),
            );
        } else {
            // Stable immutable lines scroll as one transcript. Appending a new
            // turn never replaces the surviving conversation or composer.
            let rows = [
                (0., "YOU", VIOLET),
                (0.2, "Connect Chrome DevTools.", INK),
                (0.8, "Find a place near the Panhandle.", INK),
                (3., "OPENCODE", MUTED),
                (3.2, "I’ll update the project config.", INK),
                (4.1, "Edit .opencode/opencode.jsonc", GREEN),
                (6., "Connected · tools discovered", GREEN),
                (9., "chrome-devtools_new_page", GREEN),
                (9.5, "chrome-devtools_take_snapshot", GREEN),
                (10.2, "Three listings. Let’s try Oak St.", INK),
                (13., "Sunny. One bedroom. $48,000/mo.", INK),
                (17., "Edit config · codemode: true", GREEN),
                (18., "Native browser call failed", [214, 150, 134]),
                (19., "Instructions updated: Code Mode", VIOLET),
                (21., "execute · discover browser tools", GREEN),
                (22., "New browser context. Same session.", INK),
                (23., "Still $48,000. We’re fucked.", INK),
            ];
            for (i, (at, line, color)) in rows.into_iter().enumerate() {
                let yy = y + 96. + i as f32 * 39. - scroll;
                let edge = ((yy - body.origin[1]) / 14.).clamp(0., 1.);
                text.draw(
                    c,
                    [x + 32., yy],
                    typed(line, t, at, if i < 3 { 0.7 } else { 0.5 }),
                    25.,
                    color,
                    false,
                    false,
                    fade(t, at) * edge,
                );
            }
        }
        Ok(())
    })?;
    fill(
        c,
        rect(x + 20., y + h - 80., w - 40., 58.),
        8.,
        [28, 29, 33],
        1.,
    );
    text.draw(
        c,
        [x + 38., y + h - 68.],
        "Ask anything…",
        22.,
        MUTED,
        false,
        false,
        1.,
    );
    let activity = if t < 24. { "working" } else { "ready" };
    text.draw(
        c,
        [x + w - 150., y + h - 67.],
        activity,
        20.,
        GREEN,
        false,
        false,
        1.,
    );
    Ok(())
}

fn room(c: &mut UiCanvas<'_>, b: Bounds) {
    c.clipped(Clip::rounded(b, 8.), |c| {
        c.fill(
            b,
            0.,
            Fill::Linear {
                from: [0., 0.],
                to: b.size,
                start: UiColor::srgb8(210, 204, 185, 255),
                end: UiColor::srgb8(169, 177, 157, 255),
            },
            1.,
        );
        fill(
            c,
            rect(
                b.origin[0],
                b.origin[1] + b.size[1] * 0.73,
                b.size[0],
                b.size[1] * 0.27,
            ),
            0.,
            [144, 129, 106],
            1.,
        );
        for offset in [0.12, 0.41] {
            let window = rect(
                b.origin[0] + b.size[0] * offset,
                b.origin[1] + b.size[1] * 0.1,
                b.size[0] * 0.22,
                b.size[1] * 0.52,
            );
            fill(c, window, 0., [245, 242, 224], 1.);
            fill(
                c,
                rect(
                    window.origin[0] + 6.,
                    window.origin[1] + 6.,
                    window.size[0] - 12.,
                    window.size[1] - 12.,
                ),
                0.,
                [176, 200, 189],
                1.,
            );
            fill(
                c,
                rect(
                    window.center()[0] - 2.,
                    window.origin[1],
                    4.,
                    window.size[1],
                ),
                0.,
                [244, 241, 226],
                1.,
            );
            fill(
                c,
                rect(window.origin[0], window.center()[1], window.size[0], 4.),
                0.,
                [244, 241, 226],
                1.,
            );
        }
        fill(
            c,
            rect(
                b.origin[0] + b.size[0] * 0.67,
                b.origin[1] + b.size[1] * 0.62,
                b.size[0] * 0.23,
                b.size[1] * 0.22,
            ),
            8.,
            [102, 119, 90],
            1.,
        );
        Ok(())
    })
    .unwrap();
}

fn browser(c: &mut UiCanvas<'_>, text: &mut Text, b: Bounds, t: f64, compact: bool) -> Result<()> {
    let [x, y] = b.origin;
    let [w, h] = b.size;
    let connected = t >= 6.;
    let reset = (18. ..21.).contains(&t);
    let results = t >= 9. && !reset;
    let detail = (13. ..18.).contains(&t);
    c.surface(
        b,
        SurfaceStyle::new(
            Fill::Solid(UiColor::srgb8(PAPER[0], PAPER[1], PAPER[2], 255)),
            16.,
        )
        .border(1., UiColor::srgb8(115, 117, 110, 255), 1.),
        1.,
    );
    c.clipped(Clip::rounded(b, 16.), |c| {
        fill(c, rect(x, y, w, 62.), 0., [36, 38, 40], 1.);
        for i in 0..3 {
            fill(
                c,
                rect(x + 24. + i as f32 * 17., y + 25., 8., 8.),
                4.,
                [112, 114, 113],
                1.,
            );
        }
        let address = if reset || !connected {
            "about:blank"
        } else if detail {
            "daxlist.example / oak-street"
        } else {
            "daxlist.example / san-francisco"
        };
        text.draw(
            c,
            [x + 96., y + 18.],
            address,
            19.,
            [195, 197, 190],
            false,
            false,
            1.,
        );
        let body = rect(x + 1., y + 63., w - 2., h - 64.);
        c.clipped(Clip::rounded(body, 0.), |c| {
            if !connected || reset {
                text.draw(
                    c,
                    [x + 40., y + 100.],
                    if reset {
                        "Reopening browser context…"
                    } else {
                        "A browser, ready when you are."
                    },
                    31.,
                    [113, 119, 109],
                    true,
                    false,
                    1.,
                );
                text.draw(
                    c,
                    [x + 40., y + 150.],
                    if reset {
                        "OpenCode session 01 is still open."
                    } else {
                        "Not connected yet"
                    },
                    23.,
                    [131, 135, 125],
                    true,
                    false,
                    1.,
                );
                return Ok(());
            }
            text.draw(
                c,
                [x + 32., y + 86.],
                "Daxlist",
                38.,
                [79, 82, 116],
                true,
                true,
                1.,
            );
            text.draw(
                c,
                [x + 193., y + 98.],
                "san francisco / apartments",
                21.,
                [118, 122, 111],
                true,
                false,
                1.,
            );
            rule(c, x + 32., y + 148., w - 64., [205, 207, 194]);
            if !results {
                text.draw(
                    c,
                    [x + 32., y + 183.],
                    "Haight-Ashbury",
                    34.,
                    BLACK,
                    true,
                    true,
                    fade(t, 6.),
                );
                text.draw(
                    c,
                    [x + 32., y + 238.],
                    "Near the Panhandle",
                    28.,
                    [103, 111, 98],
                    true,
                    false,
                    fade(t, 6.3),
                );
                fill(
                    c,
                    rect(x + 32., y + 310., w - 64., 60.),
                    6.,
                    [223, 224, 212],
                    1.,
                );
                text.draw(
                    c,
                    [x + 50., y + 323.],
                    "1 bedroom · search apartments",
                    25.,
                    [91, 99, 88],
                    true,
                    false,
                    1.,
                );
            } else if detail {
                let ry = if compact { y + 172. } else { y + 272. };
                let image = if compact {
                    rect(x + w - 310., ry, 274., 180.)
                } else {
                    rect(x + 32., ry, w - 64., 244.)
                };
                room(c, image);
                text.draw(
                    c,
                    [x + 32., y + 169.],
                    "Sunny one-bedroom",
                    37.,
                    BLACK,
                    true,
                    true,
                    1.,
                );
                text.draw(
                    c,
                    [x + 32., y + 222.],
                    "Oak Street · 620 ft²",
                    25.,
                    [103, 111, 98],
                    true,
                    false,
                    1.,
                );
                text.draw(
                    c,
                    [x + 32., if compact { y + 286. } else { y + 550. }],
                    "$48,000 / month",
                    40.,
                    [79, 82, 116],
                    true,
                    true,
                    fade(t, 13.2),
                );
                if !compact {
                    text.draw(
                        c,
                        [x + 32., y + 626.],
                        "Separate kitchen. Tall windows.",
                        28.,
                        BLACK,
                        true,
                        false,
                        fade(t, 13.5),
                    );
                    text.draw(
                        c,
                        [x + 32., y + 671.],
                        "A spectacularly fictional price.",
                        24.,
                        [115, 120, 108],
                        true,
                        false,
                        fade(t, 13.6),
                    );
                }
            } else {
                text.draw(
                    c,
                    [x + 32., y + 166.],
                    "3 places near the Panhandle",
                    29.,
                    BLACK,
                    true,
                    true,
                    fade(t, if t >= 21. { 21. } else { 9. }),
                );
                for (i, (name, price, street)) in [
                    ("Sunny one-bedroom", "$48,000", "Oak Street"),
                    ("Garden flat + patio", "$87,500", "Fell Street"),
                    ("Top-floor apartment", "$125,000", "Page Street"),
                ]
                .into_iter()
                .enumerate()
                {
                    let at = if t >= 21. { 21.3 } else { 9.2 } + i as f64 * 0.15;
                    let yy = y + 222. + i as f32 * if compact { 50. } else { 151. };
                    if !compact {
                        room(c, rect(x + 32., yy, 158., 116.));
                    }
                    let tx = x + if compact { 32. } else { 214. };
                    text.draw(
                        c,
                        [tx, yy],
                        name,
                        if compact { 26. } else { 29. },
                        [73, 81, 113],
                        true,
                        false,
                        fade(t, at),
                    );
                    if compact {
                        text.draw(
                            c,
                            [x + w - 280., yy],
                            price,
                            26.,
                            BLACK,
                            true,
                            true,
                            fade(t, at),
                        );
                    } else {
                        text.draw(
                            c,
                            [tx, yy + 42.],
                            price,
                            31.,
                            BLACK,
                            true,
                            true,
                            fade(t, at),
                        );
                        text.draw(
                            c,
                            [tx, yy + 86.],
                            street,
                            22.,
                            [113, 119, 105],
                            true,
                            false,
                            fade(t, at),
                        );
                        rule(c, x + 32., yy + 136., w - 64., [208, 210, 198]);
                    }
                }
            }
            Ok(())
        })?;
        Ok(())
    })?;
    text.draw(
        c,
        [x + 4., y + h + 12.],
        if t >= 21. {
            "browser context 02 · recreated"
        } else {
            "browser context 01"
        },
        20.,
        MUTED,
        false,
        false,
        1.,
    );
    Ok(())
}

fn bridge(c: &mut UiCanvas<'_>, text: &mut Text, s: Bounds, b: Bounds, t: f64) {
    let start = [s.right() - 140., s.bottom()];
    let end = [b.origin[0] + 190., b.origin[1]];
    let middle = (start[1] + end[1]) * 0.5;
    let points = [start, [start[0], middle], [end[0], middle], end];
    let color = if (17. ..21.).contains(&t) {
        VIOLET
    } else {
        GREEN
    };
    for pair in points.windows(2) {
        let [a, b] = [pair[0], pair[1]];
        fill(
            c,
            rect(
                a[0].min(b[0]) - 1.,
                a[1].min(b[1]),
                (a[0] - b[0]).abs() + 2.,
                (a[1] - b[1]).abs() + 2.,
            ),
            1.,
            color,
            0.65,
        );
    }
    let label = if t < 3. {
        "One session"
    } else if t < 6. {
        "Configuration changes"
    } else if t < 9. {
        "Browser tools arrive"
    } else if t < 17. {
        "Browser call / page result"
    } else if t < 21. {
        "Instructions refreshed"
    } else {
        "New browser · same session"
    };
    text.draw(c, [130., 503.], label, 31., color, true, false, 1.);
    if (6. ..24.).contains(&t) {
        let length = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
        let total = points
            .windows(2)
            .map(|pair| length(pair[0], pair[1]))
            .sum::<f32>();
        let mut remaining = ((t - 6.) / 1.4).fract() as f32 * total;
        for pair in points.windows(2) {
            let distance = length(pair[0], pair[1]);
            if remaining <= distance {
                let u = remaining / distance;
                let point =
                    std::array::from_fn::<_, 2, _>(|i| pair[0][i] + (pair[1][i] - pair[0][i]) * u);
                fill(c, rect(point[0] - 4., point[1] - 4., 8., 8.), 4., color, 1.);
                break;
            }
            remaining -= distance;
        }
    }
}

fn frame(variant: Variant, timeline: &Timeline, text: &mut Text, t: f64) -> Result<Vec<u8>> {
    let mut pixels = [5_u8, 6, 8, 255].repeat((SIZE[0] * SIZE[1]) as usize);
    let mut c = UiCanvas::new(&mut pixels, SIZE);
    let s = bounds(timeline, "session", t);
    let b = bounds(timeline, "browser", t);
    if !matches!(variant, Variant::Bridge) {
        text.draw(
            &mut c,
            [66., 65.],
            "A browser, without starting over.",
            43.,
            INK,
            true,
            true,
            1.,
        );
        text.draw(
            &mut c,
            [66., 998.],
            "Same OpenCode session. New capabilities.",
            26.,
            MUTED,
            true,
            false,
            1.,
        );
    }
    if matches!(variant, Variant::Bridge) {
        bridge(&mut c, text, s, b, t);
    }
    let scroll = timeline
        .sample_at(&PropertyId::new("session.scroll"), t)
        .unwrap()
        .position;
    session(
        &mut c,
        text,
        s,
        t,
        matches!(variant, Variant::Bridge),
        scroll,
    )?;
    browser(&mut c, text, b, t, matches!(variant, Variant::Bridge))?;
    Ok(pixels)
}

fn png(path: &Path, pixels: &[u8]) -> Result<()> {
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(fs::File::create(path)?),
        SIZE[0],
        SIZE[1],
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    Ok(())
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let output = Path::new(
        args.first()
            .context("usage: session-story-prototype <output> [--stills]")?,
    );
    fs::create_dir_all(output)?;
    let mut text = Text::new()?;
    for variant in [Variant::Paired, Variant::Focus, Variant::Bridge] {
        let plan = plan(variant)?;
        fs::write(
            output.join(format!("{}.plan.json", variant.id())),
            serde_json::to_vec_pretty(&plan)?,
        )?;
        let timeline = compile_channels(
            plan.continuous_channels
                .iter()
                .map(|c| (c, PropertyId::new(&c.id))),
            plan.duration_nanos,
            |s| match s {
                kinograph::plan::ScalarPlan::Literal(v) => Ok(*v),
                _ => anyhow::bail!("literal prototype"),
            },
        )?;
        for at in [1.5, 7., 10.5, 14.5, 19.5, 23.5] {
            png(
                &output.join(format!("{}-{at}.png", variant.id())),
                &frame(variant, &timeline, &mut text, at)?,
            )?;
        }
        png(
            &output.join(format!("{}.png", variant.id())),
            &frame(variant, &timeline, &mut text, 10.5)?,
        )?;
        if args.iter().any(|arg| arg == "--stills") {
            continue;
        }
        let mut encoder = encode::FfmpegEncoder::start_with_media(
            &output.join(format!("{}.mp4", variant.id())),
            encode::VideoSpec {
                width: SIZE[0],
                height: SIZE[1],
                fps: FPS,
            },
            &[],
        )?;
        for index in 0..(DURATION * FPS as f64) as u32 {
            encoder.write_frame(&frame(
                variant,
                &timeline,
                &mut text,
                index as f64 / FPS as f64,
            )?)?;
        }
        encoder.finish()?;
        println!("Rendered {} — {}", variant.id(), variant.name());
    }
    Ok(())
}

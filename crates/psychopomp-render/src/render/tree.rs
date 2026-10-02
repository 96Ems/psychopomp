//! Tree pixels: CommitMono rows on a fixed column grid, fold chevrons that
//! turn with their node's fold, indent guides, folded summaries, highlight
//! bars, and scalars that roll through their row's window when they change.
//! The layout comes from `TreeModel::layout`; this module only paints it.
use psychopomp::{
    math::{Vec2, remap_clamp, shapes::segment_distance, smoothstep, vec2},
    tree::{TreeChannel, TreeLabel, TreeLine, TreeModel, TreeNode, TreeNodeKind, TreePlan},
};
use serde_json::Value;

use super::{
    HeadlessRenderer, PlainTextSpec, TextDraw, VerticalMask, blend_pixel, composite_text,
    theme::{Palette, mix},
    ui::{
        Bounds,
        card::{Clip, Fill, UiCanvas, UiColor},
    },
};

/// Soft edge of a room opening or the scroll window, in rows.
const ROOM_FADE: f32 = 0.15;
const WINDOW_FADE: f32 = 0.6;
/// Edge fade of a value's own window while it rolls.
const ROLL_FADE: f32 = 0.2;
/// Revealed rows rise into place from this far below, in rows.
const RISE: f32 = 0.22;
const GUIDE_ALPHA: f32 = 0.32;
const BAR_ALPHA: f32 = 0.13;

/// Channel property names per node, formatted once.
pub(crate) struct TreeNames {
    nodes: Vec<NodeNames>,
}

struct NodeNames {
    open: String,
    highlight: String,
    value: String,
    expanded: f32,
}

impl TreeNames {
    pub(crate) fn new(plan: &TreePlan, model: &TreeModel) -> Self {
        let name = |path: &str, channel| psychopomp::tree::node_property(path, channel);
        Self {
            nodes: model
                .nodes
                .iter()
                .map(|node| NodeNames {
                    open: name(&node.path, TreeChannel::Open),
                    highlight: name(&node.path, TreeChannel::Highlight),
                    value: name(&node.path, TreeChannel::Value),
                    expanded: f32::from(u8::from(plan.expanded.contains(&node.path))),
                })
                .collect(),
        }
    }
}

/// Pixel geometry shared by every row of one sample.
struct Grid {
    origin: [f32; 2],
    row: f32,
    column: f32,
    scroll: f32,
    indent: f32,
    /// Columns that fit in the plan's width.
    columns: usize,
    /// The scroll window in tree rows, if any.
    window: Option<[f32; 2]>,
}

impl Grid {
    fn x(&self, column: f32) -> f32 {
        self.origin[0] + column * self.column
    }

    fn y(&self, row: f32) -> f32 {
        self.origin[1] + (row - self.scroll) * self.row
    }

    /// Where a line may draw: its opened room within the scroll window.
    fn mask(&self, clip: [f32; 2]) -> Option<VerticalMask> {
        let (top, bottom, fade) = match self.window {
            Some([top, bottom]) => (clip[0].max(top), clip[1].min(bottom), WINDOW_FADE),
            None if clip[0].is_finite() => (clip[0], clip[1], ROOM_FADE),
            None => return None,
        };
        let (top, bottom) = (self.y(top), self.y(bottom));
        (bottom > top).then(|| VerticalMask {
            top,
            bottom,
            fade: (fade * self.row).min((bottom - top) * 0.5),
        })
    }
}

/// Opacity of a revealed row from its ancestors' fold.
fn presence(reveal: f32) -> f32 {
    smoothstep(remap_clamp(reveal, [0.0, 0.8], [0.0, 1.0]))
}

/// A row fades in as room opens for it, so an opening never shows
/// half-cut glyphs at full strength.
fn line_alpha(line: &TreeLine) -> f32 {
    presence(line.reveal) * smoothstep(remap_clamp(line.room(), [0.25, 1.0], [0.0, 1.0]))
}

fn scalar_color(palette: &Palette, value: &Value) -> [u8; 3] {
    match value {
        Value::String(_) => palette.string,
        Value::Number(_) => palette.types,
        _ => palette.keyword,
    }
}

impl HeadlessRenderer {
    pub(crate) fn composite_tree(
        &mut self,
        pixels: &mut [u8],
        plan: &TreePlan,
        model: &TreeModel,
        names: &TreeNames,
        sample: impl Fn(&str, f32) -> f32,
    ) {
        let opacity = sample("opacity", 1.0).clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return;
        }
        let column = self.plain_text_sprite("0", tree_spec(plan, [0; 3])).advance;
        let scroll = sample("scroll", 0.0);
        let grid = Grid {
            origin: [
                plan.origin[0] + sample("x", 0.0),
                plan.origin[1] + sample("y", 0.0),
            ],
            row: plan.row_height(),
            column,
            scroll,
            indent: plan.indent as f32,
            columns: (plan.width / column).floor() as usize,
            window: plan.max_rows.map(|rows| [scroll, scroll + rows as f32]),
        };
        let opens = model
            .nodes
            .iter()
            .zip(&names.nodes)
            .map(|(node, names)| {
                if node.foldable() {
                    sample(&names.open, names.expanded)
                } else {
                    0.0
                }
            })
            .collect::<Vec<_>>();
        let layout = model.layout(&opens);
        let visible = |line: &TreeLine| {
            line.room() > 0.0
                && grid
                    .window
                    .is_none_or(|[from, to]| line.y + 1.0 > from && line.y < to)
        };
        // Surfaces first, so no bar or guide paints over a glyph.
        for line in layout.lines.iter().filter(|line| visible(line)) {
            let names = &names.nodes[line.node];
            let highlight = sample(&names.highlight, 0.0).clamp(0.0, 1.0);
            let alpha = opacity * line_alpha(line);
            if !line.closing && highlight > 0.001 {
                self.tree_bar(pixels, plan, &grid, line, alpha * highlight);
            }
            if !line.closing && opens[line.node] > 0.001 {
                self.tree_guide(
                    pixels,
                    model,
                    &grid,
                    line,
                    alpha * presence(opens[line.node]),
                );
            }
        }
        for line in layout.lines.iter().filter(|line| visible(line)) {
            let names = &names.nodes[line.node];
            let row = TreeRow {
                node: &model.nodes[line.node],
                line,
                alpha: opacity * line_alpha(line),
                open: opens[line.node].clamp(0.0, 1.0),
                highlight: sample(&names.highlight, 0.0).clamp(0.0, 1.0),
                variant: sample(&names.value, 0.0),
            };
            self.tree_row(pixels, plan, &grid, &row);
        }
    }

    /// The accent bar behind a highlighted row: a quiet wash and a solid edge.
    fn tree_bar(
        &mut self,
        pixels: &mut [u8],
        plan: &TreePlan,
        grid: &Grid,
        line: &TreeLine,
        alpha: f32,
    ) {
        let [r, g, b] = self.theme.palette().accent;
        let top = grid.y(line.y);
        let left = grid.x(-0.6);
        let bounds = Bounds {
            origin: [left, top + 1.0],
            size: [plan.width + grid.column * 0.6, grid.row - 2.0],
        };
        let canvas = [self.spec.width, self.spec.height];
        clipped(pixels, canvas, grid.mask(line.clip), |canvas| {
            canvas.fill(
                bounds,
                6.0,
                Fill::Solid(UiColor::srgb8(r, g, b, 255)),
                alpha * BAR_ALPHA,
            );
            canvas.fill(
                Bounds {
                    origin: bounds.origin,
                    size: [3.0, bounds.size[1]],
                },
                1.5,
                Fill::Solid(UiColor::srgb8(r, g, b, 255)),
                alpha,
            );
        });
    }

    /// A hairline under an open node's chevron, from its row to its closing
    /// bracket, inside the room it has opened.
    fn tree_guide(
        &mut self,
        pixels: &mut [u8],
        model: &TreeModel,
        grid: &Grid,
        line: &TreeLine,
        alpha: f32,
    ) {
        let node = &model.nodes[line.node];
        let x = grid.x(node.depth as f32 * grid.indent + 0.5);
        let from = line.y + 1.0;
        let to = line.y + line.extent - 1.0;
        if to <= from {
            return;
        }
        let [r, g, b] = self.theme.palette().muted;
        let canvas = [self.spec.width, self.spec.height];
        let clip = [line.clip[0].max(from), line.clip[1].min(to)];
        clipped(pixels, canvas, grid.mask(clip), |canvas| {
            canvas.fill(
                Bounds {
                    origin: [x - 0.5, grid.y(from) + 4.0],
                    size: [1.0, (to - from) * grid.row - 8.0],
                },
                0.0,
                Fill::Solid(UiColor::srgb8(r, g, b, 255)),
                alpha * GUIDE_ALPHA,
            );
        });
    }

    fn tree_row(&mut self, pixels: &mut [u8], plan: &TreePlan, grid: &Grid, row: &TreeRow<'_>) {
        if row.alpha <= 0.001 {
            return;
        }
        let palette = self.theme.palette();
        let rise = (1.0 - presence(row.line.reveal)) * RISE * grid.row;
        let pen = Pen {
            plan,
            grid,
            center: grid.y(row.line.y) + grid.row * 0.5 + rise,
            mask: grid.mask(row.line.clip),
        };
        let depth = row.node.depth as f32 * grid.indent;
        let mut column = depth + 2.0;
        if row.line.closing {
            let (_, close) = row.node.brackets().expect("closing rows close a container");
            let close = close.to_string();
            self.tree_text(pixels, &pen, column, &close, palette.muted, row.alpha);
            return;
        }
        if row.node.foldable() {
            let color = mix(palette.muted, palette.accent, row.highlight);
            self.tree_chevron(pixels, &pen, depth, row.open, color, row.alpha);
        }
        let label = match &row.node.label {
            TreeLabel::Root => None,
            TreeLabel::Key(key) => Some((
                serde_json::to_string(key).expect("strings serialize"),
                palette.text,
            )),
            TreeLabel::Index(index) => Some((index.to_string(), palette.muted)),
        };
        if let Some((label, color)) = label {
            // The key takes the accent by crossfade, so no tinted sprite per frame.
            let base = row.alpha * (1.0 - row.highlight);
            self.tree_text(pixels, &pen, column, &label, color, base);
            let lit = row.alpha * row.highlight;
            self.tree_text(pixels, &pen, column, &label, palette.accent, lit);
            column += label.chars().count() as f32;
            self.tree_text(pixels, &pen, column, ":", palette.muted, row.alpha);
            column += 2.0;
        }
        let room = grid.columns.saturating_sub(column as usize);
        match (&row.node.kind, row.node.brackets()) {
            (TreeNodeKind::Scalar { values }, _) => {
                self.tree_scalar(pixels, &pen, row, column, values, room);
            }
            (_, Some((open, close))) if !row.node.foldable() => {
                let empty = format!("{open}{close}");
                self.tree_text(pixels, &pen, column, &empty, palette.muted, row.alpha);
            }
            (_, Some((open, _))) => {
                let open_bracket = open.to_string();
                self.tree_text(
                    pixels,
                    &pen,
                    column,
                    &open_bracket,
                    palette.muted,
                    row.alpha,
                );
                // The folded tail fades as the room opens; nothing slides.
                let folded = 1.0 - smoothstep(remap_clamp(row.open, [0.0, 0.4], [0.0, 1.0]));
                if let Some((ellipsis, count)) = row.node.summary() {
                    let tail = row.alpha * folded;
                    self.tree_text(pixels, &pen, column + 1.0, &ellipsis, palette.muted, tail);
                    self.tree_text(
                        pixels,
                        &pen,
                        column + 4.0,
                        &count,
                        palette.muted,
                        tail * 0.8,
                    );
                }
            }
            _ => {}
        }
    }

    /// A scalar, rolling up through its row's window between variants.
    fn tree_scalar(
        &mut self,
        pixels: &mut [u8],
        pen: &Pen<'_>,
        row: &TreeRow<'_>,
        column: f32,
        values: &[Value],
        room: usize,
    ) {
        let palette = self.theme.palette();
        let variant = row.variant.clamp(0.0, (values.len() - 1) as f32);
        let from = variant.floor() as usize;
        let progress = variant - from as f32;
        let mut draw = |renderer: &mut Self, value: &Value, pen: &Pen<'_>| {
            let text = truncate(&psychopomp::tree::scalar_text(value), room);
            let color = scalar_color(&palette, value);
            renderer.tree_text(pixels, pen, column, &text, color, row.alpha);
        };
        if progress <= 1e-4 {
            draw(self, &values[from], pen);
            return;
        }
        let row_height = pen.grid.row;
        let window = VerticalMask {
            top: pen.center - row_height * 0.5,
            bottom: pen.center + row_height * 0.5,
            fade: ROLL_FADE * row_height,
        };
        let mask = Some(pen.mask.map_or(window, |mask| intersect(mask, window)));
        let outgoing = Pen {
            center: pen.center - progress * row_height,
            mask,
            ..*pen
        };
        let incoming = Pen {
            center: pen.center + (1.0 - progress) * row_height,
            mask,
            ..*pen
        };
        draw(self, &values[from], &outgoing);
        draw(self, &values[from + 1], &incoming);
    }

    /// A stroked chevron that turns from pointing right (folded) to down (open).
    fn tree_chevron(
        &mut self,
        pixels: &mut [u8],
        pen: &Pen<'_>,
        column: f32,
        open: f32,
        color: [u8; 3],
        alpha: f32,
    ) {
        let (grid, center, mask) = (pen.grid, pen.center, pen.mask);
        let size = grid.row / 1.6;
        let arm = size * 0.2;
        let width = (size * 0.075).max(1.2);
        let angle = open * std::f32::consts::FRAC_PI_2;
        let (sin, cos) = angle.sin_cos();
        let pivot = vec2(grid.x(column + 0.5), center);
        let turn = |p: Vec2| pivot + vec2(p.x * cos - p.y * sin, p.x * sin + p.y * cos);
        let points = [
            turn(vec2(-arm * 0.5, -arm)),
            turn(vec2(arm * 0.5, 0.0)),
            turn(vec2(-arm * 0.5, arm)),
        ];
        let canvas = [self.spec.width as i32, self.spec.height as i32];
        let reach = arm + width;
        let [r, g, b] = color;
        for y in ((pivot.y - reach).floor() as i32).max(0)
            ..((pivot.y + reach).ceil() as i32).min(canvas[1])
        {
            let coverage_y = mask.map_or(1.0, |mask| mask.coverage(y as f32, y as f32 + 1.0));
            if coverage_y <= 0.0 {
                continue;
            }
            for x in ((pivot.x - reach).floor() as i32).max(0)
                ..((pivot.x + reach).ceil() as i32).min(canvas[0])
            {
                let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
                let distance = segment_distance(p, points[0], points[1])
                    .min(segment_distance(p, points[1], points[2]));
                let coverage = (width * 0.5 + 0.5 - distance).clamp(0.0, 1.0) * coverage_y;
                if coverage <= 0.0 {
                    continue;
                }
                let index = (y as usize * canvas[0] as usize + x as usize) * 4;
                blend_pixel(
                    &mut pixels[index..index + 4],
                    [r, g, b, 255],
                    alpha * coverage,
                );
            }
        }
    }

    fn tree_text(
        &mut self,
        pixels: &mut [u8],
        pen: &Pen<'_>,
        column: f32,
        text: &str,
        color: [u8; 3],
        alpha: f32,
    ) {
        if alpha <= 0.001 || text.is_empty() {
            return;
        }
        let canvas = [self.spec.width, self.spec.height];
        let sprite = self.plain_text_sprite(text, tree_spec(pen.plan, color));
        let origin = [pen.grid.x(column), pen.center - sprite.height as f32 * 0.5];
        composite_text(
            pixels,
            canvas,
            TextDraw {
                opacity: alpha,
                mask: pen.mask,
                ..TextDraw::new(sprite, origin)
            },
        );
    }
}

/// Where one row's text goes: its vertical center and the window it may ink.
#[derive(Clone, Copy)]
struct Pen<'a> {
    plan: &'a TreePlan,
    grid: &'a Grid,
    center: f32,
    mask: Option<VerticalMask>,
}

struct TreeRow<'a> {
    node: &'a TreeNode,
    line: &'a TreeLine,
    alpha: f32,
    open: f32,
    highlight: f32,
    variant: f32,
}

fn tree_spec(plan: &TreePlan, color: [u8; 3]) -> PlainTextSpec {
    PlainTextSpec {
        font_size: plan.size,
        color,
        size: [2400, (plan.size * 1.5).ceil() as u32],
        semibold: false,
        crop_to_advance: true,
    }
}

fn intersect(a: VerticalMask, b: VerticalMask) -> VerticalMask {
    let top = a.top.max(b.top);
    let bottom = a.bottom.min(b.bottom).max(top);
    VerticalMask {
        top,
        bottom,
        fade: a.fade.max(b.fade).min((bottom - top) * 0.5),
    }
}

/// `text` cut to `columns` characters, ending in an ellipsis when cut.
fn truncate(text: &str, columns: usize) -> String {
    if text.chars().count() <= columns {
        return text.to_owned();
    }
    let mut cut = text
        .chars()
        .take(columns.saturating_sub(1))
        .collect::<String>();
    cut.push('…');
    cut
}

/// Run `draw` on a canvas clipped to `mask`'s rows (hard edges).
fn clipped(
    pixels: &mut [u8],
    size: [u32; 2],
    mask: Option<VerticalMask>,
    draw: impl FnOnce(&mut UiCanvas<'_>),
) {
    let mut canvas = UiCanvas::new(pixels, size);
    let Some(mask) = mask else {
        draw(&mut canvas);
        return;
    };
    let bounds = Bounds {
        origin: [0.0, mask.top],
        size: [size[0] as f32, mask.bottom - mask.top],
    };
    canvas
        .clipped(Clip::rounded(bounds, 0.0), |canvas| {
            draw(canvas);
            Ok(())
        })
        .expect("drawing cannot fail");
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn long_values_truncate_to_their_columns() {
        assert_eq!(truncate("\"short\"", 10), "\"short\"");
        assert_eq!(truncate("\"a long value\"", 6), "\"a lo…");
    }
}

#[cfg(test)]
mod gpu_tests {
    use psychopomp::tree::{TreeModel, TreePlan};
    use serde_json::json;

    use super::TreeNames;
    use crate::render::{HeadlessRenderer, RenderSpec};

    #[test]
    #[ignore = "requires a headless GPU; opening a node leaves rows above untouched and settles to the open pose"]
    fn opening_moves_only_rows_below_and_values_roll_inside_their_row() {
        let mut renderer = pollster::block_on(HeadlessRenderer::new(RenderSpec {
            width: 1920,
            height: 1080,
            file_name: "tree-proof".into(),
        }))
        .unwrap();
        let mut plan = TreePlan::new(
            [200.0, 100.0],
            900.0,
            json!({"actors": [{"id": "a"}, {"id": "b"}], "count": 1, "zeta": "z"}),
        )
        .expanded(["$"]);
        plan.changes.push(psychopomp::tree::TreeChangePlan {
            path: "$.count".into(),
            value: json!(2),
        });
        let model = TreeModel::new(&plan.value, &plan.changes).unwrap();
        let names = TreeNames::new(&plan, &model);
        let background = renderer.render_title_card("", None, 0.);
        let mut draw =
            |open: f32, value: f32| {
                let mut pixels = background.clone();
                renderer.composite_tree(&mut pixels, &plan, &model, &names, |property, d| {
                    match property {
                        "node.$.actors.open" => open,
                        "node.$.count.value" => value,
                        _ => d,
                    }
                });
                pixels
            };
        let closed = draw(0.0, 0.0);
        let half = draw(0.5, 0.0);
        let open = draw(1.0, 0.0);
        assert!(closed != background && closed != half && half != open);
        let row = plan.row_height();
        let rows = |pixels: &[u8], from: f32, to: f32| {
            let (from, to) = (from as usize * 1920 * 4, to as usize * 1920 * 4);
            pixels[from..to].to_vec()
        };
        // The root row above is identical in every pose.
        let above = 100.0 + row;
        assert_eq!(rows(&closed, 100.0, above), rows(&half, 100.0, above));
        assert_eq!(rows(&closed, 100.0, above), rows(&open, 100.0, above));
        // A rolling value stays inside its own row.
        let rolling = draw(0.0, 0.5);
        let count_top = 100.0 + 2.0 * row;
        assert_eq!(
            rows(&rolling, 0.0, count_top),
            rows(&closed, 0.0, count_top)
        );
        assert_eq!(
            rows(&rolling, count_top + row + 1.0, 1080.0),
            rows(&closed, count_top + row + 1.0, 1080.0)
        );
        assert!(
            rows(&rolling, count_top, count_top + row) != rows(&closed, count_top, count_top + row)
        );
        assert!(
            draw(1.0, 1.0) == draw(1.0, 1.0),
            "sampling is deterministic"
        );
    }
}

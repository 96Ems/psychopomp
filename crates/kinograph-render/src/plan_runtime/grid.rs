use anyhow::{Context, Result};
use kinograph::{
    grid::{GRID_RECIPE, GridArrangement, GridFillPlan, GridRecipePlan, GridSnapshotPlan},
    plan::{ActorPlan, ContinuousChannelPlan, ScenePlan},
    timeline::{PropertyId, Timeline},
};

use crate::render::{GridFrame, GridItemFrame, GridTextDisclosure, HeadlessRenderer};

mod disclosure;
mod table;

const CELL: f32 = 150.;

/// One concrete finite-product recipe; not a scene graph or a mesh importer.
pub(super) struct PreparedGrid {
    actor_id: String,
    recipe: GridRecipePlan,
    items: Vec<Item>,
}

struct Item {
    key: String,
    label: String,
    detail: String,
    color: [f32; 3],
    kind: Kind,
}

enum Kind {
    Cell([usize; 3]),
    Group {
        left: bool,
        index: usize,
    },
    Heading {
        axis: usize,
        index: usize,
        group: Option<(bool, usize)>,
    },
}

#[derive(Clone, Copy)]
struct Pose {
    center: [f32; 3],
    size: [f32; 3],
    presence: f32,
    emphasis: f32,
}

impl PreparedGrid {
    pub(super) fn new(actor: &ActorPlan, duration: u64) -> Result<Self> {
        let recipe: GridRecipePlan = serde_json::from_value(actor.data.clone())
            .with_context(|| format!("parse keyed grid '{}'", actor.id))?;
        recipe.validate(duration)?;
        let mut items = recipe
            .cells()?
            .into_iter()
            .map(|cell| {
                let label = recipe
                    .labels
                    .iter()
                    .find(|label| label.indices == cell.indices);
                Item {
                    key: cell.key(),
                    label: label
                        .map_or_else(|| cell.values[0].clone(), |label| label.primary.clone()),
                    detail: label.map_or_else(
                        || format!("{} · {}", cell.values[1], cell.values[2]),
                        |label| label.secondary.clone(),
                    ),
                    color: [0.96, 0.29, 0.047],
                    kind: Kind::Cell(cell.indices),
                }
            })
            .collect::<Vec<_>>();
        for (left, axis) in [(true, 2), (false, 0)] {
            if recipe.style.as_ref().is_some_and(|s| s.table.is_some()) {
                break;
            }
            for (index, value) in recipe.axes[axis].values.iter().enumerate() {
                items.push(Item {
                    key: format!("group.{}.{index}", if left { "left" } else { "right" }),
                    label: format!("{} = {value}", recipe.axes[axis].name),
                    detail: String::new(),
                    color: [0.55, 0.6, 0.68],
                    kind: Kind::Group { left, index },
                });
                for heading_axis in if left { [0, 1] } else { [1, 2] } {
                    for heading_index in 0..recipe.axes[heading_axis].values.len() {
                        items.push(heading(
                            &recipe,
                            heading_axis,
                            heading_index,
                            Some((left, index)),
                        ));
                    }
                }
            }
        }
        for axis in 0..3 {
            if axis != 0 && recipe.style.as_ref().is_some_and(|s| s.table.is_some()) {
                continue;
            }
            // A one-layer product uses Z only as the neutral one-element axis.
            // Do not make its label look like an extra column heading in 2D.
            if axis == 2 && recipe.axes[axis].values.len() == 1 {
                continue;
            }
            for index in 0..recipe.axes[axis].values.len() {
                let mut item = heading(&recipe, axis, index, None);
                if let Some(table) = recipe.style.as_ref().and_then(|s| s.table.as_ref())
                    && let Some(label) = table.headers.get(index)
                {
                    item.label = label.clone();
                }
                items.push(item);
            }
        }
        Ok(Self {
            actor_id: actor.id.clone(),
            recipe,
            items,
        })
    }

    fn pose(&self, item: &Item, snapshot: &GridSnapshotPlan) -> Pose {
        let dims = self.recipe.dimensions();
        if let Some(table) = self.recipe.style.as_ref().and_then(|s| s.table.as_ref()) {
            return table::pose(table, dims, &item.kind, snapshot.visible);
        }
        match item.kind {
            Kind::Cell([a, b, c]) => {
                let center = match snapshot.arrangement {
                    GridArrangement::Table | GridArrangement::Layers => [
                        (a as f32 - (dims[0] - 1) as f32 / 2.) * CELL,
                        ((dims[1] - 1) as f32 / 2. - b as f32) * CELL,
                        ((dims[2] - 1) as f32 / 2. - c as f32) * CELL,
                    ],
                    GridArrangement::LeftAssociated | GridArrangement::RightAssociated => {
                        let left = snapshot.arrangement == GridArrangement::LeftAssociated;
                        let (group, col, row, columns, rows) = if left {
                            (c, a, b, dims[0], dims[1])
                        } else {
                            (a, b, c, dims[1], dims[2])
                        };
                        let (center, _) = group_bounds(dims, left, group);
                        [
                            center[0] + (col as f32 - (columns - 1) as f32 / 2.) * CELL,
                            center[1] + ((rows - 1) as f32 / 2. - row as f32) * CELL - 24.,
                            0.,
                        ]
                    }
                };
                Pose {
                    center,
                    size: [CELL; 3],
                    presence: if [a, b, c].iter().zip(snapshot.visible).all(|(&i, n)| i < n) {
                        1.
                    } else {
                        0.
                    },
                    // Focus is a physical cutaway, not a contrast treatment.
                    // The outgoing layer remains the visible front until it is
                    // clipped away, so its grid lines must keep normal contrast.
                    emphasis: 1.,
                }
            }
            Kind::Heading { axis, index, group } => self.heading_pose(axis, index, group, snapshot),
            Kind::Group { left, index } => {
                let (center, size) = group_bounds(dims, left, index);
                Pose {
                    center: [center[0], center[1] + size[1] / 2. + 45., center[2]],
                    size: [size[0], 40., 0.],
                    emphasis: 1.,
                    presence: if matches!(
                        (left, snapshot.arrangement),
                        (true, GridArrangement::LeftAssociated)
                            | (false, GridArrangement::RightAssociated)
                    ) {
                        1.
                    } else {
                        0.
                    },
                }
            }
        }
    }

    fn heading_pose(
        &self,
        axis: usize,
        index: usize,
        group: Option<(bool, usize)>,
        snapshot: &GridSnapshotPlan,
    ) -> Pose {
        let dims = self.recipe.dimensions();
        let grouped = matches!(
            snapshot.arrangement,
            GridArrangement::LeftAssociated | GridArrangement::RightAssociated
        );
        let (center, visible) = if let Some((left, group_index)) = group {
            let (group_center, _) = group_bounds(dims, left, group_index);
            let (column_axis, row_axis) = if left { (0, 1) } else { (1, 2) };
            let center = if axis == column_axis {
                [
                    group_center[0] + (index as f32 - (dims[column_axis] - 1) as f32 / 2.) * CELL,
                    group_center[1] + dims[row_axis] as f32 * CELL / 2. + 10.,
                    0.,
                ]
            } else {
                [
                    group_center[0] - dims[column_axis] as f32 * CELL / 2. - 65.,
                    group_center[1] + ((dims[row_axis] - 1) as f32 / 2. - index as f32) * CELL
                        - 24.,
                    0.,
                ]
            };
            (
                center,
                if left {
                    snapshot.arrangement == GridArrangement::LeftAssociated
                } else {
                    snapshot.arrangement == GridArrangement::RightAssociated
                },
            )
        } else {
            let front = (dims[2] as f32 / 2. - snapshot.focus_slice.unwrap_or(0) as f32) * CELL;
            let center = match axis {
                0 => [
                    (index as f32 - (dims[0] - 1) as f32 / 2.) * CELL,
                    dims[1] as f32 * CELL / 2. + 38.,
                    front,
                ],
                1 => [
                    -(dims[0] as f32) * CELL / 2. - 65.,
                    ((dims[1] - 1) as f32 / 2. - index as f32) * CELL,
                    front,
                ],
                _ => [
                    (-(dims[0] as f32) / 2. + snapshot.visible[0] as f32) * CELL + 48.,
                    dims[1] as f32 * CELL / 2. + 38.,
                    ((dims[2] - 1) as f32 / 2. - index as f32) * CELL,
                ],
            };
            let depth_visible = axis != 2
                || match snapshot.focus_slice {
                    Some(slice) => index == slice,
                    None => snapshot.arrangement == GridArrangement::Layers || index == 0,
                };
            (
                center,
                !grouped && index < snapshot.visible[axis] && depth_visible,
            )
        };
        Pose {
            center,
            size: [120., 40., 0.],
            presence: if visible { 1. } else { 0. },
            emphasis: 1.,
        }
    }

    pub(super) fn channels(&self) -> Vec<ContinuousChannelPlan> {
        let mut channels = Vec::new();
        for item in &self.items {
            for (axis, name) in ["x", "y", "z"].iter().enumerate() {
                channels.push(self.channel(format!("{}.{}", item.key, name), |s| {
                    self.pose(item, s).center[axis]
                }));
            }
            channels.push(self.channel(format!("{}.presence", item.key), |s| {
                self.pose(item, s).presence
            }));
            channels.push(self.channel(format!("{}.emphasis", item.key), |s| {
                self.pose(item, s).emphasis
            }));
            channels.push(
                self.channel(format!("{}.disclosure", item.key), |snapshot| {
                    // Every tuple keeps its own ink. Real depth occlusion decides
                    // which board is visible, including the moving cut surface.
                    // Fading the outgoing board early exposes a blank opaque face.
                    if matches!(item.kind, Kind::Cell(_)) {
                        return 1.;
                    }
                    // In the edge-linked treatment, extents alone disclose new
                    // rows/columns/layers. Opacity handles only semantic switches
                    // (camera/slice/regroup), never a parallel fade of every label.
                    let mut complete = snapshot.clone();
                    complete.visible = self.recipe.dimensions();
                    self.pose(item, &complete).presence
                }),
            );
        }
        for (axis, name) in ["x", "y", "z"].iter().enumerate() {
            channels.push(self.channel(format!("extent.{name}"), |s| s.visible[axis] as f32));
        }
        channels.push(self.channel("slice.start".into(), |s| s.focus_slice.unwrap_or(0) as f32));
        channels.push(self.channel("slice.end".into(), |s| {
            s.focus_slice.map_or(s.visible[2], |slice| slice + 1) as f32
        }));
        channels.push(self.channel("grouping".into(), |s| {
            if matches!(
                s.arrangement,
                GridArrangement::LeftAssociated | GridArrangement::RightAssociated
            ) {
                1.
            } else {
                0.
            }
        }));
        channels.push(self.channel("yaw".into(), |s| {
            if s.arrangement == GridArrangement::Layers {
                0.62
            } else {
                0.
            }
        }));
        channels.push(self.channel("pitch".into(), |s| {
            if s.arrangement == GridArrangement::Layers {
                0.58
            } else {
                0.
            }
        }));
        channels
    }

    fn channel(
        &self,
        property: String,
        value: impl Fn(&GridSnapshotPlan) -> f32,
    ) -> ContinuousChannelPlan {
        let initial = value(&self.recipe.initial);
        let threshold = if !property.starts_with("extent.")
            && [".x", ".y", ".z"]
                .iter()
                .any(|suffix| property.ends_with(suffix))
        {
            0.001
        } else {
            0.000001
        };
        let motion = kinograph::plan::SpringPlan {
            response_seconds: if property.ends_with(".disclosure") {
                0.22 * 1.2
            } else {
                0.6
            },
            damping_ratio: 1.,
            position_threshold: threshold,
            velocity_threshold: threshold,
        };
        kinograph::plan::destination_channel(
            &self.actor_id,
            format!("__grid.{property}"),
            initial,
            kinograph::plan::effective_snapshots(&self.recipe.events, |e| e.at_nanos)
                .map(|e| (e.at_nanos, value(&e.snapshot))),
            |_, _| motion,
        )
    }

    pub(super) fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub(super) fn render(
        &self,
        renderer: &mut HeadlessRenderer,
        timeline: &Timeline,
        time: f64,
        presentation_scale: f32,
    ) -> Result<Vec<u8>> {
        // Direct property lookup avoids an O(cells × channels) scan per frame.
        let value = |name: &str| -> Result<f32> {
            timeline
                .sample_at(
                    &PropertyId::new(format!("{}.__grid.{name}", self.actor_id)),
                    time,
                )
                .map(|state| state.position)
                .context("missing generated grid channel")
        };
        let extents = [value("extent.x")?, value("extent.y")?, value("extent.z")?];
        let slice = [value("slice.start")?, value("slice.end")?];
        let style = self.recipe.style.as_ref();
        let table = style.and_then(|s| s.table.as_ref());
        let items = self
            .items
            .iter()
            .map(|item| {
                let pose = self.pose(item, &self.recipe.initial);
                let trim = if let Kind::Cell([_, _, c]) = item.kind {
                    [0., 0., (slice[0] - c as f32).clamp(0., 1.)]
                } else {
                    [0.; 3]
                };
                let mut reveal = if let Kind::Cell(indices) = item.kind {
                    std::array::from_fn(|axis| (extents[axis] - indices[axis] as f32).clamp(0., 1.))
                } else {
                    [1.; 3]
                };
                if let Kind::Cell([_, _, c]) = item.kind {
                    reveal[2] = ((slice[1] - c as f32).clamp(0., 1.) - trim[2]).max(0.);
                }
                Ok(GridItemFrame {
                    label: &item.label,
                    detail: &item.detail,
                    color: item.color,
                    size: pose.size,
                    center: [
                        value(&format!("{}.x", item.key))?,
                        value(&format!("{}.y", item.key))?,
                        value(&format!("{}.z", item.key))?,
                    ],
                    presence: value(&format!("{}.presence", item.key))?.clamp(0., 1.),
                    reveal,
                    trim,
                    fill: if let Kind::Cell([a, b, _]) = item.kind {
                        let srgb = |color: [u8; 3]| {
                            color.map(|c| {
                                let c = f32::from(c) / 255.;
                                if c <= 0.04045 {
                                    c / 12.92
                                } else {
                                    ((c + 0.055) / 1.055).powf(2.4)
                                }
                            })
                        };
                        match style
                            .map(|s| &s.fill)
                            .unwrap_or(&GridFillPlan::Checkerboard)
                        {
                            GridFillPlan::Checkerboard => {
                                if (a + b) % 2 == 0 {
                                    [0.032, 0.04, 0.056]
                                } else {
                                    [0.018, 0.024, 0.036]
                                }
                            }
                            GridFillPlan::None => [0.009, 0.012, 0.020],
                            GridFillPlan::Uniform { color } => srgb(*color),
                            GridFillPlan::Banded { color } => {
                                if b % 2 == 0 {
                                    srgb(*color)
                                } else {
                                    [0.009, 0.012, 0.020]
                                }
                            }
                        }
                    } else {
                        [0.; 3]
                    },
                    label_opacity: 1.,
                    text_disclosure: Some(GridTextDisclosure {
                        opacity: value(&format!("{}.disclosure", item.key))?.clamp(0., 1.)
                            * if table.is_some() && matches!(item.kind, Kind::Heading { .. }) {
                                0.7
                            } else {
                                1.
                            },
                        clip: table.map_or_else(
                            || disclosure::clip(&item.kind, extents, slice),
                            |table| table::clip(table, &item.kind, extents),
                        ),
                        feather_pixels: 8.,
                    }),
                    emphasis: value(&format!("{}.emphasis", item.key))?.clamp(0., 1.),
                    group: matches!(
                        item.kind,
                        Kind::Group { .. } | Kind::Heading { group: Some(_), .. }
                    ),
                    heading: matches!(item.kind, Kind::Heading { .. }),
                    label_style: table.and_then(|t| table::label_style(t, &item.kind)),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        // Scale fits the catalog; the renderer centers the currently sampled
        // geometry, including fractional growth and cutaways.
        let dims = self.recipe.dimensions();
        let width = ((dims[0] + dims[2]) as f32 * CELL).max(800.);
        let height = ((dims[1] + dims[2]) as f32 * CELL * 0.8).max(600.);
        let mut scale = (1400. / width).min(620. / height).min(1.25);
        for left in [true, false] {
            for index in 0..dims[if left { 2 } else { 0 }] {
                let (center, size) = group_bounds(dims, left, index);
                scale = scale.min(700. / (center[0].abs() + size[0] / 2.));
                scale = scale.min(310. / (center[1].abs() + size[1] / 2.));
            }
        }
        if let Some(table) = table {
            scale = (1400. / table.column_widths.iter().sum::<f32>())
                .min(600. / ((dims[1] + 1) as f32 * table.row_height))
                .min(1.);
        }
        let frame = GridFrame {
            items: &items,
            yaw: value("yaw")?,
            pitch: value("pitch")?,
            scale: scale * (1. - 0.24 * value("grouping")?) * presentation_scale.max(0.001),
        };
        if style.is_some() {
            renderer.render_grid_styled(frame, style)
        } else {
            renderer.render_grid(frame)
        }
    }
}

fn heading(
    recipe: &GridRecipePlan,
    axis: usize,
    index: usize,
    group: Option<(bool, usize)>,
) -> Item {
    let prefix = match group {
        Some((left, index)) => format!("group.{}.{index}", if left { "left" } else { "right" }),
        None => "grid".into(),
    };
    Item {
        key: format!("{prefix}.heading.{axis}.{index}"),
        label: recipe.axes[axis].values[index].clone(),
        detail: String::new(),
        color: [0.55, 0.6, 0.68],
        kind: Kind::Heading { axis, index, group },
    }
}

fn group_bounds(dims: [usize; 3], left: bool, index: usize) -> ([f32; 3], [f32; 3]) {
    let (count, columns, rows) = if left {
        (dims[2], dims[0], dims[1])
    } else {
        (dims[0], dims[1], dims[2])
    };
    let across = if left {
        (count as f32).sqrt().ceil() as usize
    } else {
        count
    };
    let down = count.div_ceil(across);
    let width = columns as f32 * CELL;
    let height = rows as f32 * CELL + 72.;
    (
        [
            (index % across) as f32 * (width + 144.) - (across - 1) as f32 * (width + 144.) / 2.,
            (down - 1) as f32 * (height + 120.) / 2. - (index / across) as f32 * (height + 120.),
            -32.,
        ],
        [width, height, 4.],
    )
}

/// Shared GPU-free preflight and preparation; generated channels are reserved.
#[allow(dead_code)] // Concrete entrypoint staged into the isolated browser host.
pub(super) fn compile(plan: &mut ScenePlan) -> Result<Option<PreparedGrid>> {
    let grid = plan
        .actors
        .iter()
        .find(|actor| actor.recipe == GRID_RECIPE)
        .map(|actor| PreparedGrid::new(actor, plan.duration_nanos))
        .transpose()?;
    if let Some(grid) = &grid {
        super::generated::extend(plan, grid.channels(), super::generated::Owner::Grid)?;
    }
    Ok(grid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_runtime::{PreparedPlan, validate_renderer_plan};
    use kinograph::plan::TrackEventPlan;
    use kinograph::playback::PlaybackCommand;
    use std::{path::Path, time::Duration};

    #[test]
    #[ignore = "requires a headless GPU; table paint, fixed headers, configurable rules, and opaque unfilled depth"]
    fn table_styles_change_paint_without_changing_retained_pixels_or_occlusion() {
        use kinograph::grid::{GridFillPlan, GridRules};
        let mut renderer =
            pollster::block_on(crate::plan_runtime::new_renderer("table-style-proof")).unwrap();
        let deck = kinograph_keyed_grid::build_style_deck().unwrap();
        let prepare = |plan: ScenePlan, renderer: &mut HeadlessRenderer| {
            validate_renderer_plan(&plan).unwrap();
            PreparedPlan::prepare(plan, Path::new("."), renderer).unwrap()
        };
        let p = prepare(deck.slides[0].plan.clone(), &mut renderer);
        let first = p.render_sample(&mut renderer, 5.).unwrap();
        let full = p.render_sample(&mut renderer, 11.).unwrap();
        assert_eq!(
            &first[300 * 1920 * 4..450 * 1920 * 4],
            &full[300 * 1920 * 4..450 * 1920 * 4],
            "headers and retained first row must stay still"
        );
        let pixel = |pixels: &[u8], x: usize, y: usize| {
            pixels[(y * 1920 + x) * 4..(y * 1920 + x) * 4 + 4].to_vec()
        };
        let background = pixel(&full, 10, 10);
        for y in [414, 498, 582, 666] {
            assert_eq!(
                pixel(&full, 700, y),
                background,
                "None fill must match the background"
            );
        }
        assert_ne!(
            pixel(&full, 850, 435),
            background,
            "normal tables include vertical separators by default"
        );
        assert_ne!(
            pixel(&full, 700, 456),
            background,
            "row rule must be visible"
        );
        let banded = prepare(deck.slides[1].plan.clone(), &mut renderer)
            .render_sample(&mut renderer, 11.)
            .unwrap();
        assert_ne!(pixel(&banded, 700, 414), pixel(&banded, 700, 498));
        for rules in [GridRules::Grid, GridRules::Rows, GridRules::None] {
            let mut plan = deck.slides[0].plan.clone();
            let actor = plan
                .actors
                .iter_mut()
                .find(|a| a.recipe == GRID_RECIPE)
                .unwrap();
            let mut recipe: GridRecipePlan = serde_json::from_value(actor.data.clone()).unwrap();
            recipe.style.as_mut().unwrap().rules = rules;
            actor.data = serde_json::to_value(recipe).unwrap();
            let pixels = prepare(plan, &mut renderer)
                .render_sample(&mut renderer, 11.)
                .unwrap();
            match rules {
                GridRules::Grid => assert_ne!(pixel(&pixels, 850, 435), background),
                GridRules::None => assert_eq!(pixel(&pixels, 700, 456), background),
                GridRules::Rows => assert_eq!(pixel(&pixels, 850, 435), background),
            }
        }
        // Removing material contrast is not permission to see labels through
        // the front layer. A hidden-payload canary must have no visible effect.
        let mut plan = deck.slides[2].plan.clone();
        let actor = plan
            .actors
            .iter_mut()
            .find(|a| a.recipe == GRID_RECIPE)
            .unwrap();
        let mut recipe: GridRecipePlan = serde_json::from_value(actor.data.clone()).unwrap();
        assert_eq!(recipe.style.as_ref().unwrap().fill, GridFillPlan::None);
        recipe.initial = GridSnapshotPlan {
            visible: recipe.dimensions(),
            arrangement: GridArrangement::Table,
            focus_slice: None,
        };
        recipe.events.clear();
        actor.data = serde_json::to_value(&recipe).unwrap();
        let expected = prepare(plan.clone(), &mut renderer)
            .render_sample(&mut renderer, 0.)
            .unwrap();
        for label in &mut recipe.labels {
            if label.indices[2] > 0 {
                label.primary = "LEAK".into();
                label.secondary = "HIDDEN".into();
            }
        }
        plan.actors
            .iter_mut()
            .find(|a| a.recipe == GRID_RECIPE)
            .unwrap()
            .data = serde_json::to_value(recipe).unwrap();
        let actual = prepare(plan, &mut renderer)
            .render_sample(&mut renderer, 0.)
            .unwrap();
        assert!(
            actual == expected,
            "unfilled material must remain opaque to rear ink"
        );
    }

    #[test]
    #[ignore = "requires a headless GPU; both table paints survive arbitrary native navigation and reversed growth"]
    fn styled_tables_retain_pixel_continuity_through_navigation() {
        let mut renderer =
            pollster::block_on(crate::plan_runtime::new_renderer("table-navigation-proof"))
                .unwrap();
        for slide in kinograph_keyed_grid::build_style_deck()
            .unwrap()
            .slides
            .into_iter()
            .take(2)
        {
            let p = PreparedPlan::prepare(slide.plan, Path::new("."), &mut renderer).unwrap();
            let mut playback = p.playback(false).unwrap();
            let mut now = Duration::ZERO;
            for command in [
                PlaybackCommand::Next,
                PlaybackCommand::Next,
                PlaybackCommand::Previous,
                PlaybackCommand::Last,
                PlaybackCommand::Previous,
                PlaybackCommand::First,
            ] {
                let boundary = crate::plan_runtime::proof::interrupt(
                    &p,
                    &mut renderer,
                    &mut playback,
                    now,
                    command,
                )
                .unwrap();
                assert!(boundary.changed);
                boundary.assert_states_within(&p, 0.001);
                boundary.assert_pixels(&p, &mut renderer);
                boundary.sample_later_and_repeat(&p, &mut renderer, 0.08);
                now += Duration::from_millis(130);
            }
            let mut reduced = p.playback(true).unwrap();
            for step in &p.plan.presentation_steps {
                let sample = reduced.sample(Duration::ZERO);
                assert!(
                    p.render_sample(&mut renderer, step.hold_nanos as f64 / 1e9)
                        .unwrap()
                        == p.render_sample_using(
                            &mut renderer,
                            sample.at_nanos as f64 / 1e9,
                            &reduced.timeline()
                        )
                        .unwrap()
                );
                reduced.command(PlaybackCommand::Next, Duration::ZERO);
            }
        }
    }

    #[test]
    fn one_layer_product_has_no_spurious_depth_heading() {
        let p = kinograph_data_modeling::build_deck().unwrap().slides[5]
            .plan
            .clone();
        let actor = p.actors.iter().find(|a| a.recipe == GRID_RECIPE).unwrap();
        let grid = PreparedGrid::new(actor, p.duration_nanos).unwrap();
        assert!(!grid.items.iter().any(|item| matches!(
            item.kind,
            Kind::Heading {
                axis: 2,
                group: None,
                ..
            }
        )));
        assert_eq!(
            grid.items
                .iter()
                .filter(|item| matches!(item.kind, Kind::Cell(_)))
                .count(),
            10
        );
    }

    #[test]
    fn slice_focus_changes_cutaway_bounds_not_cell_contrast() {
        let p = plan(0);
        let grid = PreparedGrid::new(&p.actors[0], p.duration_nanos).unwrap();
        for snapshot in std::iter::once(&grid.recipe.initial)
            .chain(grid.recipe.events.iter().map(|event| &event.snapshot))
        {
            for item in &grid.items {
                assert_eq!(
                    grid.pose(item, snapshot).emphasis,
                    1.,
                    "{} must keep normal contrast until geometry removes it",
                    item.key
                );
            }
        }
        for channel in grid
            .channels()
            .iter()
            .filter(|channel| channel.property.ends_with(".emphasis"))
        {
            assert!(
                channel.events.is_empty(),
                "slice focus must not schedule a competing contrast fade: {}",
                channel.id
            );
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; compare cutaway pixels with identical geometry at normal contrast"]
    fn cutaway_strokes_keep_normal_contrast_through_navigation() {
        let mut renderer =
            pollster::block_on(crate::plan_runtime::new_renderer("slice-contrast-proof")).unwrap();
        let p = prepared(0);
        let grid = &p.grid;
        let mut reference_plan = p.plan.clone();
        for channel in &mut reference_plan.continuous_channels {
            if channel.property.ends_with(".emphasis") {
                channel.initial = 1.0.into();
                channel.events.clear();
            }
        }
        let reference =
            crate::plan_runtime::CompiledPlan::compile(reference_plan, Path::new(".")).unwrap();
        let check =
            |renderer: &mut HeadlessRenderer, timeline: &Timeline, normal: &Timeline, time| {
                let pixels = grid.render(renderer, timeline, time, 1.).unwrap();
                let expected = grid.render(renderer, normal, time, 1.).unwrap();
                let changed = pixels
                    .iter()
                    .zip(expected)
                    .filter(|(a, b)| **a != *b)
                    .count();
                assert_eq!(
                    changed, 0,
                    "visible grid dims during cutaway at {time}: {changed} components"
                );
            };
        for time in [21., 21.15, 21.35, 21.65, 22., 23., 24.15, 24.35, 24.65, 26.] {
            check(&mut renderer, &p.timeline, &reference.timeline, time);
        }
        let mut playback = p.playback(true).unwrap();
        let mut normal = reference.playback(true).unwrap();
        for _ in 0..6 {
            playback.command(PlaybackCommand::Next, Duration::ZERO);
            normal.command(PlaybackCommand::Next, Duration::ZERO);
        }
        playback.set_reduced_motion(false, Duration::ZERO);
        normal.set_reduced_motion(false, Duration::ZERO);
        let mut now = Duration::ZERO;
        for command in [
            PlaybackCommand::Next,
            PlaybackCommand::Previous,
            PlaybackCommand::Last,
            PlaybackCommand::Previous,
            PlaybackCommand::Next,
        ] {
            assert!(playback.command(command, now));
            assert!(normal.command(command, now));
            for millis in [0, 40, 100] {
                let at = playback
                    .sample(now + Duration::from_millis(millis))
                    .at_nanos as f64
                    / 1e9;
                check(&mut renderer, &playback.timeline(), &normal.timeline(), at);
            }
            now += Duration::from_millis(130);
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; outgoing regroup headings cannot darken or occlude the moving grid"]
    fn regroup_headings_fade_without_dark_glyph_residue() {
        let mut renderer =
            pollster::block_on(crate::plan_runtime::new_renderer("regroup-heading-proof")).unwrap();
        let p = prepared(1);
        let grid = &p.grid;
        let mut bare = PreparedGrid::new(&plan(1).actors[0], p.plan.duration_nanos).unwrap();
        for item in &mut bare.items {
            if !matches!(item.kind, Kind::Cell(_)) {
                item.label.clear();
                item.detail.clear();
            }
        }
        let check = |renderer: &mut HeadlessRenderer, timeline: &Timeline, time| {
            let pixels = grid.render(renderer, timeline, time, 1.).unwrap();
            let base = bare.render(renderer, timeline, time, 1.).unwrap();
            // Heading green is at least as bright as every cell/stroke/label in
            // this scene. An alpha overlay cannot produce a dark green remnant.
            let dark = pixels
                .chunks_exact(4)
                .zip(base.chunks_exact(4))
                .filter(|(pixel, base)| pixel[1] < base[1])
                .count();
            assert_eq!(dark, 0, "dark heading residue at {time}: {dark} pixels");
            pixels
        };
        for time in [0., 3.15, 3.35, 6.15, 9.15, 12.05, 12.15, 12.35, 12.75, 14.] {
            check(&mut renderer, &p.timeline, time);
        }
        let mut playback = p.playback(false).unwrap();
        let mut now = Duration::ZERO;
        for command in [
            PlaybackCommand::Next,
            PlaybackCommand::Next,
            PlaybackCommand::Last,
            PlaybackCommand::Previous,
            PlaybackCommand::First,
            PlaybackCommand::Next,
        ] {
            now += Duration::from_millis(130);
            let at = playback.sample(now).at_nanos as f64 / 1e9;
            let before = check(&mut renderer, &playback.timeline(), at);
            assert!(playback.command(command, now));
            let timeline = playback.timeline();
            assert_eq!(
                before,
                check(&mut renderer, &timeline, at),
                "interruption boundary"
            );
            check(&mut renderer, &timeline, at + 0.08);
            assert_eq!(
                before,
                check(&mut renderer, &timeline, at),
                "out-of-order sampling"
            );
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; cell ink must survive the complete board-II cutaway"]
    fn slice_cutaway_never_leaves_a_blank_front_face() {
        let mut renderer =
            pollster::block_on(crate::plan_runtime::new_renderer("slice-ink-proof")).unwrap();
        let p = prepared(0);
        let grid = &p.grid;
        let mut blank = PreparedGrid::new(&plan(0).actors[0], p.plan.duration_nanos).unwrap();
        for item in &mut blank.items {
            if matches!(item.kind, Kind::Cell(_)) {
                item.label.clear();
                item.detail.clear();
            }
        }
        let ink = |renderer: &mut HeadlessRenderer, timeline: &Timeline, time| {
            let pixels = grid.render(renderer, timeline, time, 1.).unwrap();
            let background = blank.render(renderer, timeline, time, 1.).unwrap();
            pixels
                .iter()
                .zip(background)
                .map(|(&a, b)| u64::from(a.abs_diff(b)))
                .sum::<u64>()
        };
        let held = ink(&mut renderer, &p.timeline, 23.);
        for time in [21., 21.1, 21.2, 21.35, 21.5, 21.75, 22., 22.5, 23.] {
            let coverage = ink(&mut renderer, &p.timeline, time);
            eprintln!("slice t={time}: ink={coverage}, held={held}");
            assert!(
                coverage > held / 2,
                "front face goes blank at {time}: {coverage}/{held}"
            );
        }
        let mut playback = p.playback(true).unwrap();
        for _ in 0..6 {
            playback.command(PlaybackCommand::Next, Duration::ZERO);
        }
        playback.set_reduced_motion(false, Duration::ZERO);
        let mut now = Duration::ZERO;
        for command in [
            PlaybackCommand::Next,
            PlaybackCommand::Previous,
            PlaybackCommand::Last,
            PlaybackCommand::Previous,
            PlaybackCommand::Next,
        ] {
            assert!(playback.command(command, now));
            let timeline = playback.timeline();
            for millis in [0, 40, 100] {
                let at = playback
                    .sample(now + Duration::from_millis(millis))
                    .at_nanos as f64
                    / 1e9;
                assert!(
                    ink(&mut renderer, &timeline, at) > held / 2,
                    "blank ink during interrupted {command:?} at {at}"
                );
            }
            now += Duration::from_millis(130);
        }
    }

    fn plan(index: usize) -> ScenePlan {
        kinograph_keyed_grid::build_deck().unwrap().slides[index]
            .plan
            .clone()
    }

    struct CompiledGrid {
        compiled: crate::plan_runtime::CompiledPlan,
        grid: PreparedGrid,
    }
    impl std::ops::Deref for CompiledGrid {
        type Target = crate::plan_runtime::CompiledPlan;
        fn deref(&self) -> &Self::Target {
            &self.compiled
        }
    }
    fn prepared(index: usize) -> CompiledGrid {
        let mut plan = plan(index);
        let grid = compile(&mut plan).unwrap().unwrap();
        let compiled = crate::plan_runtime::CompiledPlan::compile(plan, Path::new(".")).unwrap();
        CompiledGrid { compiled, grid }
    }

    #[test]
    fn grid_preflight_rejects_invalid_recipes_roots_and_channel_collisions() {
        let mut p = plan(0);
        validate_renderer_plan(&p).unwrap();
        p.actors[0].data["initial"]["visible"] = serde_json::json!([4, 2, 4]);
        assert!(validate_renderer_plan(&p).is_err());
        let mut p = plan(0);
        let mut duplicate = p.actors[0].clone();
        duplicate.id = "other-grid".into();
        p.actors.push(duplicate);
        assert!(
            validate_renderer_plan(&p)
                .unwrap_err()
                .to_string()
                .contains("one keyed-grid")
        );
        for id_collision in [false, true] {
            let mut p = plan(0);
            let channel = PreparedGrid::new(&p.actors[0], p.duration_nanos)
                .unwrap()
                .channels()
                .remove(0);
            p.continuous_channels.push(ContinuousChannelPlan {
                id: if id_collision {
                    channel.id
                } else {
                    "alias".into()
                },
                actor_id: if id_collision {
                    "title".into()
                } else {
                    channel.actor_id
                },
                property: if id_collision {
                    "unrelated".into()
                } else {
                    channel.property
                },
                initial: 0.0.into(),
                events: vec![],
            });
            assert!(
                validate_renderer_plan(&p)
                    .unwrap_err()
                    .to_string()
                    .contains("collides")
            );
        }
        let mut p = plan(0);
        p.actors[0].data["events"][0]["atNanos"] = serde_json::json!(u64::MAX);
        assert!(validate_renderer_plan(&p).is_err());
    }

    #[test]
    fn equal_time_grid_snapshots_coalesce_and_growth_keeps_retained_cells_still() {
        let p = plan(0);
        let mut grid = PreparedGrid::new(&p.actors[0], p.duration_nanos).unwrap();
        let events = grid.recipe.events.clone();
        grid.recipe.events.insert(
            0,
            kinograph::grid::GridEventPlan {
                at_nanos: events[0].at_nanos,
                snapshot: GridSnapshotPlan {
                    visible: [3, 2, 4],
                    arrangement: GridArrangement::RightAssociated,
                    focus_slice: None,
                },
            },
        );
        let channels = grid.channels();
        grid.recipe.events = events;
        assert_eq!(
            serde_json::to_value(&channels).unwrap(),
            serde_json::to_value(grid.channels()).unwrap()
        );
        for channel in channels
            .iter()
            .filter(|channel| channel.property.starts_with("__grid.cell.0.0.0"))
        {
            // Growth never changes this tuple's geometry or presence.
            assert!(channel.events.iter().all(|event| match event {
                TrackEventPlan::Spring { at_nanos, .. } => *at_nanos >= 15_000_000_000,
                _ => false,
            }));
        }
    }

    #[test]
    fn cells_share_boundaries_and_view_changes_only_rotate_the_camera() {
        let p = plan(0);
        let grid = PreparedGrid::new(&p.actors[0], p.duration_nanos).unwrap();
        let flat = GridSnapshotPlan {
            visible: [3, 2, 4],
            arrangement: GridArrangement::Table,
            focus_slice: None,
        };
        let angled = GridSnapshotPlan {
            arrangement: GridArrangement::Layers,
            ..flat.clone()
        };
        let cell = |indices| {
            grid.items
                .iter()
                .find(|item| matches!(item.kind,Kind::Cell(i) if i == indices))
                .unwrap()
        };
        for item in grid
            .items
            .iter()
            .filter(|item| matches!(item.kind, Kind::Cell(_)))
        {
            let a = grid.pose(item, &flat);
            let b = grid.pose(item, &angled);
            assert_eq!(a.center, b.center);
            assert_eq!(a.size, [CELL; 3]);
            assert_eq!(a.presence, b.presence);
        }
        let origin = grid.pose(cell([0, 0, 0]), &flat);
        for (axis, indices) in [[1, 0, 0], [0, 1, 0], [0, 0, 1]].into_iter().enumerate() {
            let next = grid.pose(cell(indices), &flat);
            assert_eq!(
                (next.center[axis] - origin.center[axis]).abs(),
                origin.size[axis],
                "adjacent cells must meet without gaps"
            );
        }
    }

    #[test]
    fn grouping_preserves_all_tuples_and_each_group_has_the_right_members() {
        let prepared = prepared(1);
        let grid = &prepared.grid;
        let cells = grid
            .items
            .iter()
            .filter(|item| matches!(item.kind, Kind::Cell(_)))
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 24);
        for event in &grid.recipe.events {
            for item in &cells {
                assert_eq!(grid.pose(item, &event.snapshot).presence, 1.);
            }
        }
        for left in [true, false] {
            let snapshot = GridSnapshotPlan {
                visible: [3, 2, 4],
                arrangement: if left {
                    GridArrangement::LeftAssociated
                } else {
                    GridArrangement::RightAssociated
                },
                focus_slice: None,
            };
            for group in 0..if left { 4 } else { 3 } {
                let (center, size) = group_bounds([3, 2, 4], left, group);
                let members = cells
                    .iter()
                    .filter(|item| {
                        let pose = grid.pose(item, &snapshot);
                        (pose.center[0] - center[0]).abs() < size[0] / 2.
                            && (pose.center[1] - center[1]).abs() < size[1] / 2.
                    })
                    .collect::<Vec<_>>();
                assert_eq!(members.len(), if left { 6 } else { 8 });
                for member in members {
                    let Kind::Cell(indices) = member.kind else {
                        unreachable!()
                    };
                    assert_eq!(indices[if left { 2 } else { 0 }], group);
                }
            }
        }
    }

    #[test]
    fn all_grid_channels_settle_at_holds_and_interrupt_with_full_motion_state() {
        for slide in 0..2 {
            let p = prepared(slide);
            for step in &p.plan.presentation_steps {
                for channel in &p.plan.continuous_channels {
                    let property = PropertyId::new(&channel.id);
                    let at = step.hold_nanos as f64 / 1e9;
                    let state = p.timeline.sample_at(&property, at).unwrap();
                    assert_eq!(state.velocity, 0., "{} at {}", channel.id, step.id);
                }
            }
            let mut playback = p.playback(false).unwrap();
            let mut now = Duration::ZERO;
            for command in [
                PlaybackCommand::Next,
                PlaybackCommand::Previous,
                PlaybackCommand::Next,
                PlaybackCommand::Next,
                PlaybackCommand::Previous,
                PlaybackCommand::Last,
                PlaybackCommand::Previous,
                PlaybackCommand::Last,
                PlaybackCommand::First,
                PlaybackCommand::Next,
                PlaybackCommand::Next,
                PlaybackCommand::Last,
            ] {
                now += Duration::from_millis(71);
                let time = playback.sample(now).at_nanos as f64 / 1e9;
                let before = playback.timeline();
                assert!(playback.command(command, now));
                let after = playback.timeline();
                for channel in &p.plan.continuous_channels {
                    let property = PropertyId::new(&channel.id);
                    assert_eq!(
                        before.sample_at(&property, time),
                        after.sample_at(&property, time),
                        "{}",
                        channel.id
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "requires a headless GPU; checks grid navigation all the way to pixels"]
    fn grid_pixels_preserve_interruption_pause_reduced_motion_and_sampling_order() {
        let mut renderer =
            pollster::block_on(crate::plan_runtime::new_renderer("grid-proof")).unwrap();
        for slide in 0..2 {
            let p = PreparedPlan::prepare(plan(slide), Path::new("."), &mut renderer).unwrap();
            let mut playback = p.playback(false).unwrap();
            let mut now = Duration::ZERO;
            for command in [
                PlaybackCommand::Next,
                PlaybackCommand::Previous,
                PlaybackCommand::Next,
                PlaybackCommand::Next,
                PlaybackCommand::Previous,
                PlaybackCommand::Last,
                PlaybackCommand::Previous,
                PlaybackCommand::Last,
                PlaybackCommand::First,
                PlaybackCommand::Next,
                PlaybackCommand::Next,
                PlaybackCommand::Last,
            ] {
                now += Duration::from_millis(83);
                let at = playback.sample(now).at_nanos as f64 / 1e9;
                let before = playback.timeline();
                let pixels = p.render_sample_using(&mut renderer, at, &before).unwrap();
                assert!(playback.command(command, now));
                let after = playback.timeline();
                assert_eq!(
                    pixels,
                    p.render_sample_using(&mut renderer, at, &after).unwrap(),
                    "command boundary"
                );
                let prior = p
                    .render_sample_using(&mut renderer, (at - 0.0001).max(0.), &before)
                    .unwrap();
                let next = p
                    .render_sample_using(&mut renderer, at + 0.0001, &after)
                    .unwrap();
                let change = prior
                    .iter()
                    .zip(&next)
                    .map(|(&a, &b)| a.abs_diff(b) as u64)
                    .sum::<u64>();
                // Twenty-four moving cells change many edge pixels even over
                // 0.2 ms. Require convergence toward the identical boundary,
                // allowing a small MSAA coverage floor, not zero finite motion.
                let fine_prior = p
                    .render_sample_using(&mut renderer, (at - 0.00001).max(0.), &before)
                    .unwrap();
                let fine_next = p
                    .render_sample_using(&mut renderer, at + 0.00001, &after)
                    .unwrap();
                let fine = fine_prior
                    .iter()
                    .zip(&fine_next)
                    .map(|(&a, &b)| a.abs_diff(b) as u64)
                    .sum::<u64>();
                assert!(
                    fine < 500_000 && fine < change / 3 + 25_000,
                    "nonconvergent pixels: coarse={change} fine={fine}; slide={slide} command={command:?} at={at}"
                );
                let later = p
                    .render_sample_using(&mut renderer, at + 0.045, &after)
                    .unwrap();
                assert_ne!(pixels, later, "navigation must actually animate");
                assert_eq!(
                    pixels,
                    p.render_sample_using(&mut renderer, at, &after).unwrap(),
                    "out-of-order sampling"
                );
            }
            now += Duration::from_millis(91);
            playback.pause(now);
            let paused = playback.sample(now);
            assert_eq!(
                paused.at_nanos,
                playback.sample(now + Duration::from_secs(30)).at_nanos
            );
            let mut reduced = p.playback(true).unwrap();
            for (index, step) in p.plan.presentation_steps.iter().enumerate() {
                if index > 0 {
                    reduced.command(PlaybackCommand::Next, Duration::ZERO);
                }
                let expected = p
                    .render_sample(&mut renderer, step.hold_nanos as f64 / 1e9)
                    .unwrap();
                let actual = p
                    .render_sample_using(&mut renderer, 0., &reduced.timeline())
                    .unwrap();
                assert_eq!(expected, actual, "reduced-motion hold {}", step.id);
            }
            // This recipe has no reduced-quality native alternative.
            renderer.set_interactive_preview(true);
            let native = p.render_sample(&mut renderer, 3.23).unwrap();
            renderer.set_interactive_preview(false);
            assert_eq!(native, p.render_sample(&mut renderer, 3.23).unwrap());
        }
    }
}

//! Provisional composable overlays. Layout is measured once, changes lower to
//! ordinary tracks, and connectors consume the currently sampled visible bounds.
use crate::render::{HeadlessRenderer, PrototypeGlyphs};
use anyhow::{Context, Result, bail};
use kinograph::{component_prototype::*, plan::ScenePlan};
use std::collections::{HashMap, HashSet};

pub(super) enum ComponentInput {
    Typeset {
        id: String,
        plan: TypesetPlan,
        width_only: bool,
    },
    Collection {
        id: String,
        plan: CollectionPlan,
    },
    Connector {
        id: String,
        plan: ConnectorPlan,
    },
}
impl ComponentInput {
    pub(super) fn parse(actor: &kinograph::plan::ActorPlan) -> Result<Self> {
        Ok(match actor.recipe.as_str() {
            TYPESET | WIDTH_TEXT => Self::Typeset {
                id: actor.id.clone(),
                plan: serde_json::from_value(actor.data.clone())?,
                width_only: actor.recipe == WIDTH_TEXT,
            },
            COLLECTION => Self::Collection {
                id: actor.id.clone(),
                plan: serde_json::from_value(actor.data.clone())?,
            },
            CONNECTOR => Self::Connector {
                id: actor.id.clone(),
                plan: serde_json::from_value(actor.data.clone())?,
            },
            _ => bail!("not a component recipe"),
        })
    }
    fn id(&self) -> &str {
        match self {
            Self::Typeset { id, .. } | Self::Collection { id, .. } | Self::Connector { id, .. } => {
                id
            }
        }
    }
}
#[cfg(test)]
fn inputs(plan: &ScenePlan) -> Result<Vec<ComponentInput>> {
    plan.actors
        .iter()
        .filter(|a| {
            matches!(
                a.recipe.as_str(),
                TYPESET | WIDTH_TEXT | COLLECTION | CONNECTOR
            )
        })
        .map(ComponentInput::parse)
        .collect()
}

struct Item {
    id: String,
    glyphs: PrototypeGlyphs,
}
struct Group {
    id: String,
    origin: [f32; 2],
    inline: bool,
    width_only: bool,
    items: Vec<Item>,
}
struct Link {
    id: String,
    recipe: ConnectorPlan,
}
#[derive(Default)]
pub(super) struct PreparedComponents {
    groups: Vec<Group>,
    links: Vec<Link>,
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    origin: [f32; 2],
    size: [f32; 2],
    opacity: f32,
}

fn catalog(items: &[TextPart], origin: [f32; 2], font_size: f32) -> Result<HashSet<&str>> {
    if items.is_empty()
        || items.len() > 32
        || origin.iter().any(|v| !v.is_finite())
        || !font_size.is_finite()
        || !(8.0..=160.).contains(&font_size)
    {
        bail!("component prototype requires 1..32 items, finite origin, and font size in [8,160]");
    }
    let mut ids = HashSet::new();
    for item in items {
        if item.id.is_empty()
            || !item
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || !ids.insert(item.id.as_str())
            || item.text.is_empty()
            || item.text.contains(['\n', '\r'])
        {
            bail!("component items need unique alphanumeric IDs and nonempty single-line text");
        }
        if !item.spans.is_empty()
            && (item.spans.iter().any(|s| s.text.is_empty())
                || item
                    .spans
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
                    != item.text)
        {
            bail!("component syntax runs must concatenate to the part's text");
        }
    }
    Ok(ids)
}
fn members(order: &[String], ids: &HashSet<&str>) -> Result<()> {
    let mut seen = HashSet::new();
    for id in order {
        if !ids.contains(id.as_str()) || !seen.insert(id) {
            bail!("unknown or duplicate component member '{id}'");
        }
    }
    Ok(())
}
fn times(times: impl Iterator<Item = u64>, duration: u64) -> Result<()> {
    let mut previous = 0;
    for at in times {
        if at == 0 || at < previous || at > duration {
            bail!("component events must be ordered, positive, and within scene duration");
        }
        previous = at;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn validate(plan: &ScenePlan) -> Result<()> {
    validate_inputs(plan, &inputs(plan)?)
}
pub(super) fn validate_inputs(plan: &ScenePlan, inputs: &[ComponentInput]) -> Result<()> {
    let mut providers = HashMap::new();
    for input in inputs {
        let names = match input {
            ComponentInput::Typeset { plan: p, .. } => {
                let ids = catalog(&p.parts, p.origin, p.font_size)?;
                members(&p.visible, &ids)?;
                times(p.events.iter().map(|e| e.at_nanos), plan.duration_nanos)?;
                for event in &p.events {
                    members(&event.visible, &ids)?;
                }
                p.parts
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect::<HashSet<_>>()
            }
            ComponentInput::Collection { plan: p, .. } => {
                let ids = catalog(&p.items, p.origin, p.font_size)?;
                if !p.gap.is_finite() || !(0.0..=300.).contains(&p.gap) {
                    bail!("collection gap must be in [0,300]");
                }
                times(p.events.iter().map(|e| e.at_nanos), plan.duration_nanos)?;
                for snapshot in
                    std::iter::once(&p.initial).chain(p.events.iter().map(|e| &e.snapshot))
                {
                    members(&snapshot.order, &ids)?;
                    if snapshot
                        .focus
                        .as_ref()
                        .is_some_and(|id| !snapshot.order.contains(id))
                    {
                        bail!("collection focus must be a visible item");
                    }
                }
                p.items.iter().map(|p| p.id.as_str()).collect()
            }
            _ => continue,
        };
        providers.insert(input.id(), names);
        if plan
            .continuous_channels
            .iter()
            .any(|c| c.actor_id == input.id() && c.property.starts_with("__component."))
        {
            bail!("authored channels cannot use the reserved __component namespace");
        }
    }
    for input in inputs {
        let ComponentInput::Connector { id, plan: link } = input else {
            continue;
        };
        for anchor in [&link.from, &link.to] {
            if !providers
                .get(anchor.actor.as_str())
                .is_some_and(|ids| ids.contains(anchor.item.as_str()))
            {
                bail!(
                    "connector '{}' has unknown text/collection anchor '{}:{}'",
                    id,
                    anchor.actor,
                    anchor.item
                );
            }
        }
    }
    Ok(())
}

pub(super) fn reserve_inputs(
    inputs: &[ComponentInput],
    reservations: &mut super::generated::Reservations,
) -> Result<()> {
    for input in inputs {
        let properties: Vec<String> = match input {
            ComponentInput::Typeset { plan, .. } => plan
                .parts
                .iter()
                .map(|p| format!("__component.{}.presence", p.id))
                .collect(),
            ComponentInput::Collection { plan, .. } => plan
                .items
                .iter()
                .flat_map(|p| {
                    ["x", "y", "presence", "focus"]
                        .map(|name| format!("__component.{}.{name}", p.id))
                })
                .collect(),
            ComponentInput::Connector { .. } => Vec::new(),
        };
        for property in properties {
            reservations.reserve(
                &format!("{}.{property}", input.id()),
                input.id(),
                &property,
                super::generated::Owner::Component,
            )?;
        }
    }
    Ok(())
}

fn track(
    plan: &mut ScenePlan,
    reservations: &mut super::generated::Reservations,
    actor: &str,
    property: String,
    initial: f32,
    targets: Vec<(u64, f32)>,
    width_only: bool,
) -> Result<()> {
    let id = format!("{actor}.{property}");
    reservations.reserve(&id, actor, &property, super::generated::Owner::Component)?;
    let channel = kinograph::plan::destination_channel(
        actor,
        property,
        initial,
        targets,
        |current, target| kinograph::plan::SpringPlan {
            response_seconds: if width_only && target < current {
                0.24
            } else {
                0.48
            },
            damping_ratio: 1.,
            position_threshold: 0.00001,
            velocity_threshold: 0.00001,
        },
    );
    plan.continuous_channels.push(channel);
    Ok(())
}

impl PreparedComponents {
    pub(super) fn prepare_inputs(
        plan: &mut ScenePlan,
        inputs: Vec<ComponentInput>,
        renderer: &mut HeadlessRenderer,
    ) -> Result<Self> {
        let mut prepared = Self::default();
        let mut reservations = super::generated::Reservations::new(&plan.continuous_channels);
        for input in inputs {
            match input {
                ComponentInput::Typeset {
                    id,
                    plan: p,
                    width_only,
                } => {
                    let mut items = Vec::new();
                    for part in p.parts {
                        track(
                            plan,
                            &mut reservations,
                            &id,
                            format!("__component.{}.presence", part.id),
                            f32::from(p.visible.contains(&part.id)),
                            p.events
                                .iter()
                                .map(|e| (e.at_nanos, f32::from(e.visible.contains(&part.id))))
                                .collect(),
                            width_only,
                        )?;
                        let glyphs = renderer.prepare_prototype_part(&part, p.font, p.font_size);
                        items.push(Item {
                            id: part.id,
                            glyphs,
                        });
                    }
                    prepared.groups.push(Group {
                        id,
                        origin: p.origin,
                        inline: true,
                        width_only,
                        items,
                    });
                }
                ComponentInput::Collection { id, plan: p } => {
                    let items = p
                        .items
                        .iter()
                        .map(|part| Item {
                            id: part.id.clone(),
                            glyphs: renderer.prepare_prototype_part(part, p.font, p.font_size),
                        })
                        .collect::<Vec<_>>();
                    let snapshots = std::iter::once((0, &p.initial))
                        .chain(p.events.iter().map(|e| (e.at_nanos, &e.snapshot)))
                        .collect::<Vec<_>>();
                    let snapshots = kinograph::plan::effective_snapshots(&snapshots, |s| s.0)
                        .copied()
                        .collect::<Vec<_>>();
                    let layouts = snapshots
                        .iter()
                        .map(|(_, snapshot)| {
                            let mut cursor = 0.;
                            snapshot
                                .order
                                .iter()
                                .map(|id| {
                                    let item = items
                                        .iter()
                                        .find(|item| &item.id == id)
                                        .expect("validated member");
                                    let position = match snapshot.arrangement {
                                        Arrangement::Row => [cursor, 0.],
                                        Arrangement::Column => [0., cursor],
                                    };
                                    cursor += match snapshot.arrangement {
                                        Arrangement::Row => item.glyphs.width(),
                                        Arrangement::Column => item.glyphs.height(),
                                    } + p.gap;
                                    (id.as_str(), position)
                                })
                                .collect::<HashMap<_, _>>()
                        })
                        .collect::<Vec<_>>();
                    for item in &items {
                        let first = layouts
                            .iter()
                            .find_map(|l| l.get(item.id.as_str()))
                            .copied()
                            .unwrap_or([0., 0.]);
                        for (axis, name) in ["x", "y"].iter().enumerate() {
                            let mut previous = first[axis];
                            let targets = snapshots
                                .iter()
                                .zip(&layouts)
                                .skip(1)
                                .map(|((at, _), layout)| {
                                    previous =
                                        layout.get(item.id.as_str()).map_or(previous, |p| p[axis]);
                                    (*at, previous)
                                })
                                .collect();
                            track(
                                plan,
                                &mut reservations,
                                &id,
                                format!("__component.{}.{name}", item.id),
                                first[axis],
                                targets,
                                false,
                            )?;
                        }
                        for (property, focus) in [("presence", false), ("focus", true)] {
                            let value = |s: &CollectionSnapshot| {
                                f32::from(if focus {
                                    s.focus.as_ref() == Some(&item.id)
                                } else {
                                    s.order.contains(&item.id)
                                })
                            };
                            track(
                                plan,
                                &mut reservations,
                                &id,
                                format!("__component.{}.{property}", item.id),
                                value(&p.initial),
                                p.events
                                    .iter()
                                    .map(|e| (e.at_nanos, value(&e.snapshot)))
                                    .collect(),
                                false,
                            )?;
                        }
                    }
                    prepared.groups.push(Group {
                        id,
                        origin: p.origin,
                        inline: false,
                        width_only: false,
                        items,
                    });
                }
                ComponentInput::Connector { id, plan: recipe } => {
                    prepared.links.push(Link { id, recipe })
                }
            }
        }
        plan.validate()?;
        Ok(prepared)
    }

    fn bounds(&self, sample: &impl Fn(&str, &str, f32) -> f32) -> HashMap<(&str, &str), Bounds> {
        let mut bounds = HashMap::new();
        for group in &self.groups {
            let origin = [
                sample(&group.id, "x", group.origin[0]),
                sample(&group.id, "y", group.origin[1]),
            ];
            let opacity = sample(&group.id, "opacity", 1.).clamp(0., 1.);
            let mut cursor = 0.;
            for item in &group.items {
                let presence = sample(&group.id, &format!("__component.{}.presence", item.id), 1.)
                    .clamp(0., 1.);
                let offset = if group.inline {
                    [cursor, 0.]
                } else {
                    [
                        sample(&group.id, &format!("__component.{}.x", item.id), 0.),
                        sample(&group.id, &format!("__component.{}.y", item.id), 0.),
                    ]
                };
                let width = item.glyphs.width() * presence;
                bounds.insert(
                    (group.id.as_str(), item.id.as_str()),
                    Bounds {
                        origin: [origin[0] + offset[0], origin[1] + offset[1]],
                        size: [width, item.glyphs.height()],
                        opacity: opacity * if group.width_only { 1. } else { presence },
                    },
                );
                cursor += width;
            }
        }
        bounds
    }

    pub(super) fn render(
        &self,
        pixels: &mut [u8],
        renderer: &HeadlessRenderer,
        sample: impl Fn(&str, &str, f32) -> f32,
    ) -> Result<()> {
        let bounds = self.bounds(&sample);
        for link in &self.links {
            let get = |a: &Anchor| {
                bounds
                    .get(&(a.actor.as_str(), a.item.as_str()))
                    .copied()
                    .context("missing measured connector anchor")
            };
            let a = get(&link.recipe.from)?;
            let b = get(&link.recipe.to)?;
            let (start, n0) = anchor(a, link.recipe.from.side);
            let (end, n1) = anchor(b, link.recipe.to.side);
            let opacity = a.opacity.min(b.opacity) * sample(&link.id, "opacity", 1.).clamp(0., 1.);
            let control = ((end[0] - start[0]).hypot(end[1] - start[1]) * 0.4).clamp(40., 180.);
            let controls = [
                start,
                [start[0] + n0[0] * control, start[1] + n0[1] * control],
                [end[0] + n1[0] * control, end[1] + n1[1] * control],
                end,
            ];
            let path = (0..=96)
                .map(|i| bezier(controls, i as f32 / 96.))
                .collect::<Vec<_>>();
            let draw = sample(&link.id, "draw", 1.).clamp(0., 1.);
            let mut ink = path_prefix(&path, draw);
            if ink.len() > 1 {
                let tip = ink[ink.len() - 1];
                let previous = ink[ink.len() - 2];
                let d = [tip[0] - previous[0], tip[1] - previous[1]];
                let length = d[0].hypot(d[1]).max(0.001);
                let t = [d[0] / length, d[1] / length];
                let arrow = 8. * draw;
                ink.extend_from_slice(&[
                    [
                        tip[0] - t[0] * arrow - t[1] * arrow * 0.55,
                        tip[1] - t[1] * arrow + t[0] * arrow * 0.55,
                    ],
                    tip,
                    [
                        tip[0] - t[0] * arrow + t[1] * arrow * 0.55,
                        tip[1] - t[1] * arrow - t[0] * arrow * 0.55,
                    ],
                ]);
                renderer.composite_prototype_path(pixels, &ink, 1.6, link.recipe.color, opacity);
            }
            let signal = sample(&link.id, "signal", 0.).clamp(0., 1.);
            let signal_opacity = sample(&link.id, "signal-opacity", 0.).clamp(0., 1.);
            if signal_opacity > 0. {
                let p = path_prefix(&path, signal * draw)
                    .last()
                    .copied()
                    .unwrap_or(start);
                renderer.composite_prototype_path(
                    pixels,
                    &[p, p],
                    8.,
                    link.recipe.accent,
                    opacity * signal_opacity * draw,
                );
            }
        }
        for group in &self.groups {
            for item in &group.items {
                let b = bounds[&(group.id.as_str(), item.id.as_str())];
                let presence = sample(&group.id, &format!("__component.{}.presence", item.id), 1.);
                if group.width_only {
                    renderer.composite_prototype_glyphs_with_blur(
                        pixels,
                        &item.glyphs,
                        b.origin,
                        presence,
                        b.opacity,
                        (1. - presence.clamp(0., 1.)) * 4.,
                    );
                } else {
                    renderer.composite_prototype_glyphs(
                        pixels,
                        &item.glyphs,
                        b.origin,
                        presence,
                        b.opacity,
                    );
                }
                if !group.inline {
                    let focus = sample(&group.id, &format!("__component.{}.focus", item.id), 0.)
                        .clamp(0., 1.);
                    let y = b.origin[1] + b.size[1] - 2.;
                    renderer.composite_prototype_path(
                        pixels,
                        &[[b.origin[0], y], [b.origin[0] + b.size[0], y]],
                        1.5,
                        [214, 164, 112],
                        b.opacity * focus,
                    );
                }
            }
        }
        Ok(())
    }
}

fn anchor(b: Bounds, side: Side) -> ([f32; 2], [f32; 2]) {
    let normal = match side {
        Side::Left => [-1., 0.],
        Side::Right => [1., 0.],
        Side::Top => [0., -1.],
        Side::Bottom => [0., 1.],
    };
    (
        [
            b.origin[0] + b.size[0] * (normal[0] + 1.) / 2. + normal[0] * 18.,
            b.origin[1] + b.size[1] * (normal[1] + 1.) / 2. + normal[1] * 18.,
        ],
        normal,
    )
}
fn bezier(p: [[f32; 2]; 4], t: f32) -> [f32; 2] {
    let u = 1. - t;
    std::array::from_fn(|i| {
        u * u * u * p[0][i]
            + 3. * u * u * t * p[1][i]
            + 3. * u * t * t * p[2][i]
            + t * t * t * p[3][i]
    })
}
fn path_prefix(path: &[[f32; 2]], fraction: f32) -> Vec<[f32; 2]> {
    let distance = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
    let mut remaining = path.windows(2).map(|p| distance(p[0], p[1])).sum::<f32>() * fraction;
    let mut result = vec![path[0]];
    if fraction <= 0. {
        return result;
    }
    for pair in path.windows(2) {
        let d = distance(pair[0], pair[1]);
        if remaining < d {
            let t = remaining / d;
            result.push([
                pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                pair[0][1] + (pair[1][1] - pair[0][1]) * t,
            ]);
            break;
        }
        result.push(pair[1]);
        remaining -= d;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_runtime::{PreparedPlan, new_renderer};
    use kinograph::playback::PlaybackCommand;
    use std::{path::Path, time::Duration};

    #[test]
    fn prototype_preflight_rejects_unknown_anchors_and_ambiguous_identity() {
        let deck = kinograph_component_prototypes::build_deck().unwrap();
        for slide in &deck.slides {
            validate(&slide.plan).unwrap();
        }
        let mut p = deck.slides[2].plan.clone();
        let link = p.actors.iter_mut().find(|a| a.recipe == CONNECTOR).unwrap();
        link.data["to"]["item"] = serde_json::json!("missing");
        assert!(validate(&p).is_err());
        let mut p = deck.slides[0].plan.clone();
        p.actors[0].data["parts"][1]["id"] = serde_json::json!("make");
        assert!(validate(&p).is_err());
        let mut p = kinograph_component_prototypes::build_slideshow_deck()
            .unwrap()
            .slides[3]
            .plan
            .clone();
        validate(&p).unwrap();
        let expression = p
            .actors
            .iter_mut()
            .find(|a| a.recipe == WIDTH_TEXT)
            .unwrap();
        expression.data["parts"][2]["spans"][0]["text"] = serde_json::json!("wrong text");
        assert!(validate(&p).is_err());
    }

    #[test]
    fn bezier_and_arc_length_disclosure_have_exact_endpoints() {
        let controls = [[0., 0.], [100., 0.], [100., 200.], [200., 200.]];
        assert_eq!(bezier(controls, 0.), controls[0]);
        assert_eq!(bezier(controls, 1.), controls[3]);
        let path = [[0., 0.], [30., 0.], [30., 70.]];
        assert_eq!(path_prefix(&path, 0.), vec![[0., 0.]]);
        assert_eq!(
            path_prefix(&path, 0.5),
            vec![[0., 0.], [30., 0.], [30., 20.]]
        );
        assert_eq!(path_prefix(&path, 1.), path);
    }

    #[test]
    #[ignore = "requires a headless GPU; typography, keyed layout, and live anchors through interrupted navigation"]
    fn component_trials_preserve_pixels_and_anchors_through_reversals() {
        let mut renderer = pollster::block_on(new_renderer("component-prototype-proof")).unwrap();
        for slide in kinograph_component_prototypes::build_deck().unwrap().slides {
            let p = PreparedPlan::prepare(slide.plan, Path::new("."), &mut renderer).unwrap();
            if p.plan.id == "collection-prototype" {
                for time in [3.05, 3.12, 3.2, 3.4, 3.8] {
                    let bounds = p
                        .components
                        .bounds(&|a, c, d| p.property_value(&p.timeline, a, c, time, d));
                    let entering = bounds[&("pipeline", "validate")];
                    let retained = bounds[&("pipeline", "persist")];
                    assert!(
                        entering.origin[0] + entering.size[0] <= retained.origin[0],
                        "new ink must fit the space currently opened at {time}"
                    );
                }
            }
            let mut playback = p.playback(false).unwrap();
            let mut now = Duration::ZERO;
            for command in [
                PlaybackCommand::Next,
                PlaybackCommand::Previous,
                PlaybackCommand::Last,
                PlaybackCommand::Previous,
                PlaybackCommand::First,
                PlaybackCommand::Next,
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
                let at = boundary.at;
                let old_bounds = p
                    .components
                    .bounds(&|a, c, d| p.property_value(&boundary.before, a, c, at, d));
                boundary.assert_states_within(&p, 0.001);
                let bounds = p
                    .components
                    .bounds(&|a, c, d| p.property_value(&boundary.after, a, c, at, d));
                for (id, b) in &bounds {
                    let a = old_bounds[id];
                    for axis in 0..2 {
                        assert!(
                            (a.origin[axis] - b.origin[axis]).abs() < 0.001
                                && (a.size[axis] - b.size[axis]).abs() < 0.001
                        );
                    }
                }
                boundary.assert_pixels(&p, &mut renderer);
                let later = boundary.sample_later_and_repeat(&p, &mut renderer, 0.09);
                assert!(later != boundary.pixels, "{} visibly changes", p.plan.id);
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
                        .unwrap(),
                    "{} reduced-motion endpoint",
                    p.plan.id
                );
                reduced.command(PlaybackCommand::Next, Duration::ZERO);
            }
        }
    }
}

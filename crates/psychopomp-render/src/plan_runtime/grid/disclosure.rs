//! Labels disclose at the sampled growth edge, not on per-label timers.
use super::{CELL, Kind};
use crate::render::GridTextClip;

pub(super) fn clip(kind: &Kind, extents: [f32; 3], slice: [f32; 2]) -> GridTextClip {
    match *kind {
        Kind::Cell(_) => GridTextClip::Cell,
        Kind::Heading {
            axis,
            index,
            group: None,
        } => {
            let center = index as f32 + 0.5;
            let (start, end) = if axis == 2 {
                (slice[0], slice[1])
            } else {
                (0., extents[axis])
            };
            GridTextClip::Heading {
                axis,
                start: (start - center) * CELL,
                end: (end - center) * CELL,
                cell: CELL,
            }
        }
        // Reassociation is not growth: do not invent a wipe across new group titles.
        _ => GridTextClip::None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::PreparedGrid;
    use super::*;

    #[test]
    fn edge_disclosure_is_spatially_staggered_not_scheduled() {
        for axis in 0..3 {
            let heading = |index| Kind::Heading {
                axis,
                index,
                group: None,
            };
            let mut extents = [4.; 3];
            extents[axis] = 1.6;
            let clip = |index| super::clip(&heading(index), extents, [0., extents[2]]);
            for (index, expected) in [(0, 1.1 * CELL), (1, 0.1 * CELL), (2, -0.9 * CELL)] {
                let GridTextClip::Heading { end, .. } = clip(index) else {
                    panic!("missing heading aperture")
                };
                assert!((end - expected).abs() < 0.0001);
            }
        }
        let plan = psychopomp_keyed_grid::build_deck().unwrap().slides[0]
            .plan
            .clone();
        let channels = PreparedGrid::new(&plan.actors[0], plan.duration_nanos)
            .unwrap()
            .channels();
        for index in 0..3 {
            let id = format!("__grid.grid.heading.0.{index}.disclosure");
            let heading = channels
                .iter()
                .find(|channel| channel.property == id)
                .unwrap();
            assert_eq!(
                serde_json::to_value(&heading.initial).unwrap(),
                serde_json::json!(1.)
            );
            assert!(
                heading.events.is_empty(),
                "growth must not start a competing fade"
            );
        }
        let depth = channels
            .iter()
            .find(|channel| channel.property == "__grid.grid.heading.2.2.disclosure")
            .unwrap();
        // The camera can disclose depth headings; growing the depth cannot.
        assert!(depth.events.iter().all(|event| match event {
            psychopomp::plan::TrackEventPlan::Spring { at_nanos, .. } =>
                ![12_000_000_000, 15_000_000_000].contains(at_nanos),
            _ => false,
        }));
    }
}

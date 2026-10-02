//! Ordinary table placement over the same cell catalog and growth tracks.
use super::{Kind, Pose};
use crate::render::{GridLabelStyle, GridTextClip};
use psychopomp::grid::{GridAlignment, GridTableLayout};

fn column_center(table: &GridTableLayout, index: usize) -> f32 {
    table.column_widths[..index].iter().sum::<f32>() + table.column_widths[index] / 2.
        - table.column_widths.iter().sum::<f32>() / 2.
}

pub(super) fn pose(
    table: &GridTableLayout,
    dims: [usize; 3],
    kind: &Kind,
    visible: [usize; 3],
) -> Pose {
    let depth = table.row_height / 2.;
    let (center, size, present) = match *kind {
        Kind::Cell([a, b, c]) => (
            [
                column_center(table, a),
                ((dims[1] - 1) as f32 / 2. - b as f32) * table.row_height,
                0.,
            ],
            [table.column_widths[a], table.row_height, depth],
            a < visible[0] && b < visible[1] && c < visible[2],
        ),
        Kind::Heading {
            axis: 0,
            index,
            group: None,
        } => (
            [
                column_center(table, index),
                dims[1] as f32 * table.row_height / 2. + table.row_height / 2.,
                depth / 2.,
            ],
            [table.column_widths[index], table.row_height, 0.],
            index < visible[0],
        ),
        _ => ([0.; 3], [1.; 3], false),
    };
    Pose {
        center,
        size,
        presence: f32::from(present),
        emphasis: 1.,
    }
}

pub(super) fn label_style(table: &GridTableLayout, kind: &Kind) -> Option<GridLabelStyle> {
    let (column, heading) = match *kind {
        Kind::Cell([a, _, _]) => (a, false),
        Kind::Heading {
            axis: 0,
            index,
            group: None,
        } => (index, true),
        _ => return None,
    };
    Some(GridLabelStyle {
        font_size: if heading {
            table.font_size * 0.8
        } else {
            table.font_size
        },
        padding: table.padding,
        alignment: table
            .alignments
            .get(column)
            .copied()
            .unwrap_or(GridAlignment::Left),
    })
}

pub(super) fn clip(table: &GridTableLayout, kind: &Kind, extents: [f32; 3]) -> GridTextClip {
    match *kind {
        Kind::Cell(_) => GridTextClip::Cell,
        Kind::Heading {
            axis: 0,
            index,
            group: None,
        } => {
            let width = table.column_widths[index];
            GridTextClip::Heading {
                axis: 0,
                start: -width / 2.,
                end: ((extents[0] - index as f32).clamp(0., 1.) - 0.5) * width,
                cell: width,
            }
        }
        _ => GridTextClip::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psychopomp::grid::GridStylePlan;

    #[test]
    fn unequal_columns_meet_and_retained_rows_and_headers_do_not_move() {
        let table = GridStylePlan::plain_table(vec![400., 280., 200.])
            .table
            .unwrap();
        for row in 0..4 {
            let a = pose(&table, [3, 4, 1], &Kind::Cell([0, row, 0]), [3, 4, 1]);
            let b = pose(&table, [3, 4, 1], &Kind::Cell([1, row, 0]), [3, 4, 1]);
            assert_eq!(a.center[0] + a.size[0] / 2., b.center[0] - b.size[0] / 2.);
        }
        for kind in [
            Kind::Cell([0, 0, 0]),
            Kind::Heading {
                axis: 0,
                index: 0,
                group: None,
            },
        ] {
            let first = pose(&table, [3, 4, 1], &kind, [1, 1, 1]);
            let last = pose(&table, [3, 4, 1], &kind, [3, 4, 1]);
            assert_eq!(first.center, last.center);
            assert_eq!(first.size, last.size);
        }
    }
}

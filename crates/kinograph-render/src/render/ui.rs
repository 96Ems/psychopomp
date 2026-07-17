pub(crate) mod card;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    Horizontal,
    Vertical,
}

#[cfg_attr(not(test), allow(dead_code))]
impl Axis {
    fn index(self) -> usize {
        match self {
            Self::Horizontal => 0,
            Self::Vertical => 1,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bounds {
    pub origin: [f32; 2],
    pub size: [f32; 2],
}

impl Bounds {
    pub fn from_center(center: [f32; 2], size: [f32; 2]) -> Self {
        Self {
            origin: [center[0] - size[0] * 0.5, center[1] - size[1] * 0.5],
            size,
        }
    }

    pub fn center(self) -> [f32; 2] {
        [
            self.origin[0] + self.size[0] * 0.5,
            self.origin[1] + self.size[1] * 0.5,
        ]
    }

    pub fn right(self) -> f32 {
        self.origin[0] + self.size[0]
    }

    pub fn bottom(self) -> f32 {
        self.origin[1] + self.size[1]
    }

    pub fn inset(self, edges: Edges) -> Self {
        Self {
            origin: [self.origin[0] + edges.left, self.origin[1] + edges.top],
            size: [
                (self.size[0] - edges.left - edges.right).max(0.0),
                (self.size[1] - edges.top - edges.bottom).max(0.0),
            ],
        }
    }

    pub fn expand(self, amount: f32) -> Self {
        self.inset(Edges::all(-amount))
    }

    pub fn translate(self, offset: [f32; 2]) -> Self {
        Self {
            origin: [self.origin[0] + offset[0], self.origin[1] + offset[1]],
            size: self.size,
        }
    }

    pub fn split_top(self, height: f32) -> (Self, Self) {
        let height = height.clamp(0.0, self.size[1]);
        (
            Self {
                origin: self.origin,
                size: [self.size[0], height],
            },
            Self {
                origin: [self.origin[0], self.origin[1] + height],
                size: [self.size[0], self.size[1] - height],
            },
        )
    }

    pub fn split_left(self, width: f32) -> (Self, Self) {
        let width = width.clamp(0.0, self.size[0]);
        (
            Self {
                origin: self.origin,
                size: [width, self.size[1]],
            },
            Self {
                origin: [self.origin[0] + width, self.origin[1]],
                size: [self.size[0] - width, self.size[1]],
            },
        )
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl Bounds {
    pub(crate) fn split_bottom(self, height: f32) -> (Self, Self) {
        let height = height.clamp(0.0, self.size[1]);
        let top_height = self.size[1] - height;
        (
            Self {
                origin: self.origin,
                size: [self.size[0], top_height],
            },
            Self {
                origin: [self.origin[0], self.origin[1] + top_height],
                size: [self.size[0], height],
            },
        )
    }

    pub(crate) fn split_right(self, width: f32) -> (Self, Self) {
        let width = width.clamp(0.0, self.size[0]);
        let left_width = self.size[0] - width;
        (
            Self {
                origin: self.origin,
                size: [left_width, self.size[1]],
            },
            Self {
                origin: [self.origin[0] + left_width, self.origin[1]],
                size: [width, self.size[1]],
            },
        )
    }

    pub(crate) fn contains(self, point: [f32; 2]) -> bool {
        point[0] >= self.origin[0]
            && point[0] <= self.right()
            && point[1] >= self.origin[1]
            && point[1] <= self.bottom()
    }

    pub(crate) fn align(self, size: [f32; 2], horizontal: Align, vertical: Align) -> Self {
        let (x, width) = align_axis(self.origin[0], self.size[0], size[0], horizontal);
        let (y, height) = align_axis(self.origin[1], self.size[1], size[1], vertical);
        Self {
            origin: [x, y],
            size: [width, height],
        }
    }

    pub(crate) fn equal_columns<const N: usize>(self, gap: f32) -> [Self; N] {
        self.equal_tracks(Axis::Horizontal, gap)
    }

    pub(crate) fn equal_rows<const N: usize>(self, gap: f32) -> [Self; N] {
        self.equal_tracks(Axis::Vertical, gap)
    }

    fn equal_tracks<const N: usize>(self, axis: Axis, gap: f32) -> [Self; N] {
        if N == 0 {
            return [self; N];
        }

        let axis = axis.index();
        let extent = self.size[axis].max(0.0);
        let gap_count = N - 1;
        let gap = if gap_count == 0 {
            0.0
        } else {
            gap.max(0.0).min(extent / gap_count as f32)
        };
        let track_extent = ((extent - gap * gap_count as f32) / N as f32).max(0.0);

        std::array::from_fn(|index| {
            let mut track = self;
            track.origin[axis] += index as f32 * (track_extent + gap);
            track.size[axis] = track_extent;
            track
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn align_axis(origin: f32, available: f32, extent: f32, align: Align) -> (f32, f32) {
    match align {
        Align::Start => (origin, extent),
        Align::Center => (origin + (available - extent) * 0.5, extent),
        Align::End => (origin + available - extent, extent),
        Align::Stretch => (origin, available),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct Flow {
    bounds: Bounds,
    axis: Axis,
    item_extent: [f32; 2],
    gap: f32,
    cross_align: Align,
    index: usize,
}

#[cfg_attr(not(test), allow(dead_code))]
impl Flow {
    pub(crate) fn new(
        bounds: Bounds,
        axis: Axis,
        item_extent: [f32; 2],
        gap: f32,
        cross_align: Align,
    ) -> Self {
        Self {
            bounds,
            axis,
            item_extent,
            gap,
            cross_align,
            index: 0,
        }
    }

    pub(crate) fn next(&mut self) -> Bounds {
        let main_axis = self.axis.index();
        let mut item = match self.axis {
            Axis::Horizontal => self
                .bounds
                .align(self.item_extent, Align::Start, self.cross_align),
            Axis::Vertical => self
                .bounds
                .align(self.item_extent, self.cross_align, Align::Start),
        };
        item.origin[main_axis] += self.index as f32 * (self.item_extent[main_axis] + self.gap);
        self.index += 1;
        item
    }
}

pub(crate) struct VerticalFlow {
    bounds: Bounds,
    line_height: f32,
    index: usize,
}

impl VerticalFlow {
    pub fn new(bounds: Bounds, line_height: f32) -> Self {
        Self {
            bounds,
            line_height,
            index: 0,
        }
    }

    pub fn next(&mut self) -> Bounds {
        let row = Bounds {
            origin: [
                self.bounds.origin[0],
                self.bounds.origin[1] + self.index as f32 * self.line_height,
            ],
            size: [self.bounds.size[0], self.line_height],
        };
        self.index += 1;
        row
    }
}

#[cfg(test)]
mod tests {
    use super::{Align, Axis, Bounds, Edges, Flow, VerticalFlow};

    #[test]
    fn bounds_compose_without_authored_child_coordinates() {
        let card = Bounds::from_center([100.0, 80.0], [160.0, 100.0]);
        let (header, body) = card.split_top(20.0);
        let (gutter, content) = body.inset(Edges::symmetric(8.0, 4.0)).split_left(24.0);
        let mut rows = VerticalFlow::new(content, 16.0);

        assert_eq!(header.origin, [20.0, 30.0]);
        assert_eq!(gutter.size, [24.0, 72.0]);
        assert_eq!(rows.next().origin, [52.0, 54.0]);
        assert_eq!(rows.next().origin, [52.0, 70.0]);
    }

    #[test]
    fn flow_places_horizontal_and_vertical_items_with_gaps_and_alignment() {
        let bounds = Bounds {
            origin: [10.0, 20.0],
            size: [100.0, 60.0],
        };
        let mut horizontal = Flow::new(bounds, Axis::Horizontal, [20.0, 10.0], 5.0, Align::Center);

        assert_eq!(
            horizontal.next(),
            Bounds {
                origin: [10.0, 45.0],
                size: [20.0, 10.0],
            }
        );
        assert_eq!(horizontal.next().origin, [35.0, 45.0]);

        let mut vertical = Flow::new(bounds, Axis::Vertical, [20.0, 16.0], 4.0, Align::End);

        assert_eq!(
            vertical.next(),
            Bounds {
                origin: [90.0, 20.0],
                size: [20.0, 16.0],
            }
        );
        assert_eq!(vertical.next().origin, [90.0, 40.0]);

        let mut stretched = Flow::new(bounds, Axis::Horizontal, [20.0, 10.0], 0.0, Align::Stretch);
        assert_eq!(stretched.next().size, [20.0, 60.0]);
    }

    #[test]
    fn bounds_split_from_trailing_edges_and_align_children() {
        let bounds = Bounds {
            origin: [10.0, 20.0],
            size: [100.0, 80.0],
        };

        let (top, bottom) = bounds.split_bottom(30.0);
        assert_eq!(top.size, [100.0, 50.0]);
        assert_eq!(bottom.origin, [10.0, 70.0]);
        assert_eq!(bottom.size, [100.0, 30.0]);

        let (left, right) = bounds.split_right(25.0);
        assert_eq!(left.size, [75.0, 80.0]);
        assert_eq!(right.origin, [85.0, 20.0]);
        assert_eq!(right.size, [25.0, 80.0]);

        assert_eq!(
            bounds.align([20.0, 10.0], Align::Center, Align::End),
            Bounds {
                origin: [50.0, 90.0],
                size: [20.0, 10.0],
            }
        );
        assert!(bounds.contains([10.0, 20.0]));
        assert!(bounds.contains([110.0, 100.0]));
        assert!(!bounds.contains([110.1, 100.0]));
    }

    #[test]
    fn equal_tracks_apply_gaps_and_handle_degenerate_counts() {
        let bounds = Bounds {
            origin: [10.0, 20.0],
            size: [100.0, 80.0],
        };

        let columns = bounds.equal_columns::<3>(5.0);
        assert_eq!(columns[0].size, [30.0, 80.0]);
        assert_eq!(columns[1].origin, [45.0, 20.0]);
        assert_eq!(columns[2].origin, [80.0, 20.0]);

        let rows = bounds.equal_rows::<3>(10.0);
        assert_eq!(rows[0].size, [100.0, 20.0]);
        assert_eq!(rows[1].origin, [10.0, 50.0]);
        assert_eq!(rows[2].origin, [10.0, 80.0]);

        let no_columns = bounds.equal_columns::<0>(f32::INFINITY);
        assert_eq!(no_columns, []);

        let compressed = Bounds {
            origin: [0.0, 0.0],
            size: [20.0, 10.0],
        }
        .equal_columns::<3>(100.0);
        assert_eq!(compressed.map(|column| column.size[0]), [0.0; 3]);
        assert_eq!(compressed.map(|column| column.origin[0]), [0.0, 10.0, 20.0]);
        assert!(
            compressed
                .iter()
                .flat_map(|column| column.origin.into_iter().chain(column.size))
                .all(f32::is_finite)
        );
    }
}

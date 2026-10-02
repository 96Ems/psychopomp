pub(crate) mod card;

/// Signed distance from `local` to a `size` rectangle centred on the origin,
/// its corners rounded by `radius` clamped to the shorter half-side.
pub(crate) fn rounded_rect_distance(local: [f32; 2], size: [f32; 2], radius: f32) -> f32 {
    let radius = radius.min(size[0].min(size[1]) * 0.5);
    let dx = local[0].abs() - (size[0] * 0.5 - radius);
    let dy = local[1].abs() - (size[1] * 0.5 - radius);
    dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - radius
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
    use super::{Bounds, Edges, VerticalFlow};

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
}

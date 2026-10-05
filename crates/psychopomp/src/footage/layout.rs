//! Collage layouts, without a GPU: grids, masonry columns, scattered and
//! piled prints with seeded turns, and filmstrips. Each returns [`Tile`]s in
//! order (a later tile lies on top of an earlier one); a Scene Program
//! staggers them in with [`by_distance`] or a plain loop. Areas are
//! `[x, y, width, height]` in canvas pixels.
use crate::math::{Vec2, random::hash, shapes::r2, vec2};

/// Where one piece of a collage goes: its center, size, and rest angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub rotation: f32,
}

impl Tile {
    /// The largest box of `aspect` (width over height) inside this tile.
    pub fn fit(self, aspect: f32) -> Self {
        let size = if self.size[0] / self.size[1] > aspect {
            [self.size[1] * aspect, self.size[1]]
        } else {
            [self.size[0], self.size[0] / aspect]
        };
        Self { size, ..self }
    }

    /// The tile moved by `offset`.
    pub fn shifted(self, offset: [f32; 2]) -> Self {
        Self {
            center: [self.center[0] + offset[0], self.center[1] + offset[1]],
            ..self
        }
    }
}

/// `count` tiles of `aspect` in `columns`, `gap` apart, as large as fits
/// `area` and centered in it; a partial last row is centered too.
pub fn grid(area: [f32; 4], count: usize, columns: usize, gap: f32, aspect: f32) -> Vec<Tile> {
    let columns = columns.clamp(1, count.max(1));
    let rows = count.div_ceil(columns);
    if count == 0 {
        return Vec::new();
    }
    let mut width = (area[2] - gap * (columns - 1) as f32) / columns as f32;
    let mut height = width / aspect;
    let tall = height * rows as f32 + gap * (rows - 1) as f32;
    if tall > area[3] {
        height = (area[3] - gap * (rows - 1) as f32) / rows as f32;
        width = height * aspect;
    }
    let block_height = height * rows as f32 + gap * (rows - 1) as f32;
    let top = area[1] + (area[3] - block_height) * 0.5;
    (0..count)
        .map(|index| {
            let (row, column) = (index / columns, index % columns);
            let in_row = (count - row * columns).min(columns);
            let row_width = width * in_row as f32 + gap * (in_row - 1) as f32;
            let left = area[0] + (area[2] - row_width) * 0.5;
            Tile {
                center: [
                    left + column as f32 * (width + gap) + width * 0.5,
                    top + row as f32 * (height + gap) + height * 0.5,
                ],
                size: [width, height],
                rotation: 0.0,
            }
        })
        .collect()
}

/// Masonry: equal-width `columns`, each piece (by its `aspects`, width over
/// height) dropped into the shortest column, `gap` apart; the whole wall
/// scales down to fit `area` and centers in it.
pub fn masonry(area: [f32; 4], aspects: &[f32], columns: usize, gap: f32) -> Vec<Tile> {
    let columns = columns.max(1);
    let width = (area[2] - gap * (columns - 1) as f32) / columns as f32;
    let mut heights = vec![0.0_f32; columns];
    let mut placed = Vec::with_capacity(aspects.len());
    for aspect in aspects {
        let column = (0..columns)
            .min_by(|&a, &b| heights[a].total_cmp(&heights[b]).then(a.cmp(&b)))
            .unwrap_or(0);
        let height = width / aspect.max(0.05);
        placed.push((column, heights[column], height));
        heights[column] += height + gap;
    }
    let tallest = heights.iter().copied().fold(0.0, f32::max) - gap;
    let scale = if tallest > area[3] {
        area[3] / tallest
    } else {
        1.0
    };
    let wall = vec2(area[2], tallest.max(0.0)) * scale;
    let origin = vec2(area[0], area[1]) + (vec2(area[2], area[3]) - wall) * 0.5;
    placed
        .into_iter()
        .map(|(column, top, height)| {
            let local = vec2(
                column as f32 * (width + gap) + width * 0.5,
                top + height * 0.5,
            );
            let center = origin + local * scale;
            Tile {
                center: center.to_array(),
                size: [width * scale, height * scale],
                rotation: 0.0,
            }
        })
        .collect()
}

/// `count` prints of `size` spread over `area` (kept inside it) on a
/// low-discrepancy pattern with seeded jitter and turns of up to `turn`
/// radians either way. The same seed gives the same table.
pub fn scatter(area: [f32; 4], count: usize, size: [f32; 2], seed: u32, turn: f32) -> Vec<Tile> {
    let half = Vec2::from(size) * 0.5;
    let inner = vec2((area[2] - size[0]).max(0.0), (area[3] - size[1]).max(0.0));
    let origin = vec2(area[0], area[1]) + half;
    let cell = inner / (count.max(1) as f32).sqrt();
    (0..count as u32)
        .map(|index| {
            let jitter = vec2(hash(index, seed ^ 0x51ed), hash(index, seed ^ 0x7a3b)) - 0.5;
            let spot =
                (r2(index + seed % 977) * inner + jitter * cell * 0.4).clamp(Vec2::ZERO, inner);
            Tile {
                center: (origin + spot).to_array(),
                size,
                rotation: (hash(index, seed ^ 0x2d9f) * 2.0 - 1.0) * turn,
            }
        })
        .collect()
}

/// `count` prints of `size` piled around `center`: most near the middle,
/// within `spread` pixels, overlapping, each turned up to `turn` radians.
pub fn pile(
    center: [f32; 2],
    count: usize,
    size: [f32; 2],
    seed: u32,
    spread: f32,
    turn: f32,
) -> Vec<Tile> {
    (0..count as u32)
        .map(|index| {
            // Square-root radius spreads evenly over the disc; the power
            // gathers prints toward the middle of the pile.
            let angle = std::f32::consts::TAU * hash(index, seed ^ 0x1b87);
            let radius = spread * hash(index, seed ^ 0x6c4f).powf(0.75);
            let offset = Vec2::from_angle(angle) * radius;
            Tile {
                center: (Vec2::from(center) + offset).to_array(),
                size,
                rotation: (hash(index, seed ^ 0x2d9f) * 2.0 - 1.0) * turn,
            }
        })
        .collect()
}

/// `count` frames of `size` in a strip from `first` (the first frame's
/// center), `gap` apart, along x or (when `vertical`) y.
pub fn filmstrip(
    first: [f32; 2],
    count: usize,
    size: [f32; 2],
    gap: f32,
    vertical: bool,
) -> Vec<Tile> {
    let step = if vertical {
        vec2(0.0, size[1] + gap)
    } else {
        vec2(size[0] + gap, 0.0)
    };
    (0..count)
        .map(|index| Tile {
            center: (Vec2::from(first) + step * index as f32).to_array(),
            size,
            rotation: 0.0,
        })
        .collect()
}

/// Tile indices nearest `from` first: an order to stagger a collage in as a
/// ripple outward from a point.
pub fn by_distance(tiles: &[Tile], from: [f32; 2]) -> Vec<usize> {
    let from = Vec2::from(from);
    let mut order = (0..tiles.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| {
        let distance = |index: usize| Vec2::from(tiles[index].center).distance(from);
        distance(a).total_cmp(&distance(b)).then(a.cmp(&b))
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(tile: &Tile, area: [f32; 4]) -> bool {
        let eps = 1e-3;
        tile.center[0] - tile.size[0] * 0.5 >= area[0] - eps
            && tile.center[1] - tile.size[1] * 0.5 >= area[1] - eps
            && tile.center[0] + tile.size[0] * 0.5 <= area[0] + area[2] + eps
            && tile.center[1] + tile.size[1] * 0.5 <= area[1] + area[3] + eps
    }

    fn overlap(a: &Tile, b: &Tile) -> bool {
        (a.center[0] - b.center[0]).abs() < (a.size[0] + b.size[0]) * 0.5 - 1e-3
            && (a.center[1] - b.center[1]).abs() < (a.size[1] + b.size[1]) * 0.5 - 1e-3
    }

    #[test]
    fn grids_fit_their_area_and_center_partial_rows() {
        let area = [100.0, 100.0, 1600.0, 800.0];
        let tiles = grid(area, 7, 3, 20.0, 16.0 / 9.0);
        assert_eq!(tiles.len(), 7);
        assert!(tiles.iter().all(|tile| inside(tile, area)));
        for (index, a) in tiles.iter().enumerate() {
            assert!((a.size[0] / a.size[1] - 16.0 / 9.0).abs() < 1e-4);
            assert!(tiles[index + 1..].iter().all(|b| !overlap(a, b)));
        }
        // The lone last tile sits under the middle column.
        assert!((tiles[6].center[0] - tiles[1].center[0]).abs() < 1e-3);
        assert!((tiles[1].center[0] - 900.0).abs() < 1e-3);
        assert!(grid(area, 0, 3, 20.0, 1.0).is_empty());
    }

    #[test]
    fn masonry_fills_the_shortest_column_and_fits() {
        let area = [0.0, 0.0, 1000.0, 600.0];
        let aspects = [1.0, 0.5, 2.0, 1.5, 0.75, 1.0, 1.78];
        let tiles = masonry(area, &aspects, 3, 10.0);
        assert_eq!(tiles.len(), aspects.len());
        for (tile, aspect) in tiles.iter().zip(aspects) {
            assert!((tile.size[0] / tile.size[1] - aspect).abs() < 1e-3);
            assert!(inside(tile, area));
        }
        for (index, a) in tiles.iter().enumerate() {
            assert!(tiles[index + 1..].iter().all(|b| !overlap(a, b)));
        }
        // The fourth piece goes under the shortest of the first three: the wide one.
        assert!((tiles[3].center[0] - tiles[2].center[0]).abs() < 1e-3);
    }

    #[test]
    fn scatters_and_piles_are_seeded_and_stay_put() {
        let area = [0.0, 0.0, 1920.0, 1080.0];
        let a = scatter(area, 12, [300.0, 200.0], 7, 0.2);
        assert_eq!(a, scatter(area, 12, [300.0, 200.0], 7, 0.2));
        assert_ne!(a, scatter(area, 12, [300.0, 200.0], 8, 0.2));
        assert!(
            a.iter()
                .all(|tile| { inside(tile, area) && tile.rotation.abs() <= 0.2 })
        );
        let pile = pile([960.0, 540.0], 9, [320.0, 220.0], 3, 140.0, 0.3);
        assert_eq!(
            pile,
            super::pile([960.0, 540.0], 9, [320.0, 220.0], 3, 140.0, 0.3)
        );
        assert!(pile.iter().all(|tile| {
            Vec2::from(tile.center).distance(vec2(960.0, 540.0)) <= 140.0 + 1e-3
                && tile.rotation.abs() <= 0.3
        }));
        // A pile overlaps: that is what makes it a pile.
        assert!(pile.iter().skip(1).any(|tile| overlap(tile, &pile[0])));
    }

    #[test]
    fn filmstrips_step_by_size_and_gap_and_ripples_order_by_distance() {
        let strip = filmstrip([200.0, 540.0], 4, [160.0, 90.0], 10.0, false);
        assert_eq!(strip[3].center, [200.0 + 3.0 * 170.0, 540.0]);
        let column = filmstrip([200.0, 100.0], 2, [160.0, 90.0], 10.0, true);
        assert_eq!(column[1].center, [200.0, 200.0]);
        assert_eq!(by_distance(&strip, [800.0, 540.0]), vec![3, 2, 1, 0]);
        let fitted = strip[0].fit(1.0);
        assert_eq!(fitted.size, [90.0, 90.0]);
        assert_eq!(strip[0].fit(4.0).size, [160.0, 40.0]);
    }
}

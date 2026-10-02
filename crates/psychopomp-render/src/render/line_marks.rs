//! Coverage union for diff backgrounds. Adjacent fractional rows share one
//! coverage field, instead of independently antialiasing and blending their seam.
use psychopomp::tone::Tone;

/// Added and removed inks, in `Band::kind` order.
pub(super) const TONES: [Tone; 2] = [Tone::Success, Tone::Error];

pub(super) struct Band {
    pub top: f32,
    pub bottom: f32,
    pub opacity: f32,
    pub kind: usize,
}

pub(super) fn row_coverage(bands: &[Band], height: u32) -> Vec<[f32; 2]> {
    let mut edges = bands
        .iter()
        .flat_map(|band| [band.top, band.bottom])
        .collect::<Vec<_>>();
    edges.sort_by(f32::total_cmp);
    edges.dedup();
    let mut rows = vec![[0.0; 2]; height as usize];
    for interval in edges.windows(2) {
        let middle = (interval[0] + interval[1]) * 0.5;
        let mut opacity = [0.0_f32; 2];
        for band in bands {
            if band.top <= middle && middle < band.bottom {
                opacity[band.kind] = opacity[band.kind].max(band.opacity);
            }
        }
        let from = interval[0].floor().max(0.0) as usize;
        let to = (interval[1].ceil().max(0.0) as usize).min(rows.len());
        for (y, row) in rows.iter_mut().enumerate().take(to).skip(from) {
            let coverage = (interval[1].min(y as f32 + 1.0) - interval[0].max(y as f32)).max(0.0);
            for kind in 0..2 {
                row[kind] += opacity[kind] * coverage;
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_adjacent_rows_have_no_internal_seams() {
        for offset in [0.0, 0.2, 0.5, 0.9] {
            let bands = [
                Band {
                    top: offset,
                    bottom: 44.0 + offset,
                    opacity: 0.8,
                    kind: 0,
                },
                Band {
                    top: 44.0 + offset,
                    bottom: 88.0 + offset,
                    opacity: 0.8,
                    kind: 0,
                },
            ];
            let rows = row_coverage(&bands, 90);
            for row in &rows[1..88] {
                assert!((row[0] - 0.8).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn overlapping_rows_do_not_stack_tint() {
        let rows = row_coverage(
            &[
                Band {
                    top: 0.0,
                    bottom: 10.0,
                    opacity: 0.4,
                    kind: 0,
                },
                Band {
                    top: 5.0,
                    bottom: 15.0,
                    opacity: 0.7,
                    kind: 0,
                },
            ],
            15,
        );
        assert_eq!(rows[7][0], 0.7);
    }
}

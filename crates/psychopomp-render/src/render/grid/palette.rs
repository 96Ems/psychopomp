//! Native line-color auditioning; these do not change fill, text, or motion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridLinePalette {
    #[default]
    Orange,
    Copper,
    Slate,
    Sage,
    Chalk,
}

impl GridLinePalette {
    pub const ALL: [Self; 5] = [
        Self::Orange,
        Self::Copper,
        Self::Slate,
        Self::Sage,
        Self::Chalk,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Orange => "Orange",
            Self::Copper => "Muted copper",
            Self::Slate => "Slate blue",
            Self::Sage => "Sage",
            Self::Chalk => "Chalk",
        }
    }

    pub fn cycle(self, reverse: bool) -> Self {
        let index = Self::ALL
            .iter()
            .position(|palette| *palette == self)
            .unwrap();
        Self::ALL[(index + if reverse { Self::ALL.len() - 1 } else { 1 }) % Self::ALL.len()]
    }

    // Linear RGB, matching the existing material shader (not sRGB byte values).
    pub(super) fn color(self) -> [f32; 3] {
        match self {
            Self::Orange => [0.96, 0.29, 0.047],
            Self::Copper => [0.42, 0.20, 0.11],
            Self::Slate => [0.22, 0.28, 0.36],
            Self::Sage => [0.20, 0.32, 0.28],
            Self::Chalk => [0.42, 0.43, 0.45],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_cycle_wraps_and_reverses() {
        let mut palette = GridLinePalette::default();
        for expected in GridLinePalette::ALL {
            assert_eq!(palette, expected);
            let next = palette.cycle(false);
            assert_eq!(next.cycle(true), palette);
            palette = next;
        }
        assert_eq!(palette, GridLinePalette::default());
    }
}

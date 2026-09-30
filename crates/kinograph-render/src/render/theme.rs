//! Paint tokens shared by native presentation and file delivery. No geometry,
//! clocks, recorded pixels, or semantic state changes live in a theme.
use super::TextSprite;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    Original,
    Evergreen,
    TokyoNight,
    Black,
    /// The OpenCode TUI's dark tokens (packages/tui theme `opencode`).
    #[serde(rename = "opencode")]
    OpenCode,
    /// The OpenCode blog's "clear neutral" diagrams: quiet frames and wires,
    /// warm ivory signals, and desaturated semantic inks used only for change.
    Neutral,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: [u8; 3],
    pub surface: [u8; 3],
    pub raised: [u8; 3],
    pub text: [u8; 3],
    pub muted: [u8; 3],
    pub accent: [u8; 3],
    pub keyword: [u8; 3],
    pub types: [u8; 3],
    pub string: [u8; 3],
}

impl Theme {
    pub const ALL: [Self; 6] = [
        Self::Original,
        Self::Evergreen,
        Self::TokyoNight,
        Self::Black,
        Self::OpenCode,
        Self::Neutral,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Original => "Original",
            Self::Evergreen => "Evergreen",
            Self::TokyoNight => "Tokyo Night",
            Self::Black => "Pure Black",
            Self::OpenCode => "OpenCode",
            Self::Neutral => "Clear Neutral",
        }
    }
    pub fn cycle(self, reverse: bool) -> Self {
        let index = Self::ALL.iter().position(|t| *t == self).unwrap();
        Self::ALL[(index + if reverse { Self::ALL.len() - 1 } else { 1 }) % Self::ALL.len()]
    }
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "original" => Ok(Self::Original),
            "evergreen" => Ok(Self::Evergreen),
            "tokyo-night" => Ok(Self::TokyoNight),
            "black" => Ok(Self::Black),
            "opencode" => Ok(Self::OpenCode),
            "neutral" => Ok(Self::Neutral),
            _ => anyhow::bail!(
                "unknown theme '{value}'; use original, evergreen, tokyo-night, black, opencode, or neutral"
            ),
        }
    }
    pub fn palette(self) -> Palette {
        match self {
            Self::Original => Palette {
                background: [1, 2, 4],
                surface: [13, 18, 27],
                raised: [31, 36, 47],
                text: [235, 233, 227],
                muted: [143, 150, 165],
                accent: [216, 168, 120],
                keyword: [196, 181, 253],
                types: [125, 211, 252],
                string: [190, 242, 100],
            },
            Self::Evergreen => Palette {
                background: [13, 23, 20],
                surface: [22, 36, 30],
                raised: [33, 49, 41],
                text: [224, 233, 219],
                muted: [151, 173, 154],
                accent: [164, 196, 144],
                keyword: [214, 181, 140],
                types: [136, 193, 177],
                string: [180, 205, 144],
            },
            Self::TokyoNight => Palette {
                background: [26, 27, 38],
                surface: [31, 35, 53],
                raised: [41, 46, 66],
                text: [192, 202, 245],
                muted: [137, 149, 190],
                accent: [122, 162, 247],
                keyword: [187, 154, 247],
                types: [125, 207, 255],
                string: [158, 206, 106],
            },
            Self::Black => Palette {
                background: [0, 0, 0],
                surface: [10, 10, 10],
                raised: [23, 23, 23],
                text: [238, 238, 238],
                muted: [156, 156, 156],
                accent: [224, 174, 115],
                keyword: [207, 181, 237],
                types: [151, 207, 223],
                string: [179, 212, 151],
            },
            Self::OpenCode => Palette {
                background: [10, 10, 10],
                surface: [20, 20, 20],
                raised: [30, 30, 30],
                text: [238, 238, 238],
                muted: [128, 128, 128],
                accent: [250, 178, 131],
                keyword: [157, 124, 216],
                types: [229, 192, 123],
                string: [127, 216, 143],
            },
            Self::Neutral => Palette {
                background: [8, 8, 7],
                surface: [14, 14, 13],
                raised: [34, 34, 33],
                text: [226, 223, 217],
                muted: [133, 133, 133],
                accent: [224, 179, 90],
                keyword: [176, 160, 204],
                types: [214, 192, 146],
                string: [168, 186, 150],
            },
        }
    }
    /// A semantic tone's color. Status tones are identical in every theme
    /// except Neutral, whose desaturated inks are reserved for change.
    pub fn tone(self, tone: kinograph::tone::Tone) -> [u8; 3] {
        use kinograph::tone::Tone;
        let palette = self.palette();
        if self == Self::Neutral {
            return match tone {
                Tone::Plain => palette.text,
                Tone::Request => [236, 233, 228],
                Tone::Success => [165, 173, 147],
                Tone::Error => [237, 129, 126],
                Tone::Warning => [224, 179, 90],
                Tone::Muted => palette.muted,
                Tone::Accent => palette.accent,
            };
        }
        match tone {
            Tone::Plain => palette.text,
            Tone::Request => [92, 156, 245],
            Tone::Success => [127, 216, 143],
            Tone::Error => [224, 108, 117],
            Tone::Warning => [229, 192, 123],
            Tone::Muted => palette.muted,
            Tone::Accent => palette.accent,
        }
    }
    pub fn background(self, original: [u8; 3]) -> [u8; 3] {
        if self == Self::Original {
            original
        } else {
            self.palette().background
        }
    }
    pub fn surface(self, original: [u8; 3]) -> [u8; 3] {
        if self == Self::Original {
            original
        } else {
            self.palette().surface
        }
    }
    /// Compatibility bridge for existing authored RGB typography. Known syntax
    /// colors retain their roles; neutral ink and the showroom accent use tokens.
    /// Other literal colors (including semantic red/green) are deliberately kept.
    pub fn ink(self, rgb: [u8; 3]) -> [u8; 3] {
        if self == Self::Original {
            return rgb;
        }
        let p = self.palette();
        match rgb {
            [196, 181, 253] => p.keyword,
            [125, 211, 252] => p.types,
            [190, 242, 100] => p.string,
            [110, 231, 183] | [216, 168, 120] | [214, 164, 112] => p.accent,
            _ => {
                let lo = *rgb.iter().min().unwrap();
                let hi = *rgb.iter().max().unwrap();
                if hi - lo <= 48 {
                    if hi >= 200 { p.text } else { p.muted }
                } else {
                    rgb
                }
            }
        }
    }
    pub(super) fn sprite(self, sprite: &mut TextSprite) {
        if self == Self::Original {
            return;
        }
        for p in sprite.pixels.chunks_exact_mut(4) {
            if p[3] > 0 {
                let rgb = self.ink([p[0], p[1], p[2]]);
                p[..3].copy_from_slice(&rgb);
            }
        }
    }
}

pub(super) fn linear(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|c| {
        let c = f32::from(c) / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn themes_cycle_round_trip_and_keep_semantic_literals() {
        for theme in Theme::ALL {
            assert_eq!(theme.cycle(false).cycle(true), theme);
            assert_eq!(
                serde_json::from_str::<Theme>(&serde_json::to_string(&theme).unwrap()).unwrap(),
                theme
            );
            assert_eq!(theme.ink([239, 68, 68]), [239, 68, 68]);
        }
        assert_eq!(Theme::Black.palette().background, [0; 3]);
        assert_eq!(Theme::parse("opencode").unwrap(), Theme::OpenCode);
        assert_eq!(Theme::OpenCode.palette().accent, [250, 178, 131]);
        assert!(Theme::parse("missing").is_err());
        assert_eq!(Theme::parse("neutral").unwrap(), Theme::Neutral);
        assert_eq!(
            Theme::OpenCode.tone(kinograph::tone::Tone::Success),
            [127, 216, 143],
            "existing themes keep their status inks"
        );
    }
}

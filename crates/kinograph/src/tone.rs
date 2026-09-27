//! Semantic color roles shared by explainer recipes such as captions and
//! sequence diagrams. Status tones keep fixed colors in every Presentation Theme;
//! the others follow the theme palette.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tone {
    #[default]
    Plain,
    /// A request or neutral action (blue).
    Request,
    Success,
    Error,
    Warning,
    Muted,
    /// The theme accent, for the one thing to follow.
    Accent,
}

impl Tone {
    pub fn is_default(&self) -> bool {
        *self == Self::Plain
    }
}

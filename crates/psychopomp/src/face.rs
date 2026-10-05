//! Typefaces a Stage label or subtitles can be set in. CommitMono is bundled
//! and the default; the others are faces macOS installs, as the Helvetica
//! Neue used for prose and headers already is, so another machine falls back
//! to whatever it has.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Face {
    /// Bundled CommitMono.
    #[default]
    Mono,
    /// Didot, a high-contrast display serif: titles set quietly and large.
    Serif,
    /// Didot Italic, for whispers.
    SerifItalic,
    /// Helvetica Neue Light.
    Light,
    /// Helvetica Neue Condensed Black: shouting.
    Shout,
}

impl Face {
    pub fn is_mono(&self) -> bool {
        *self == Self::Mono
    }
}

#[cfg(test)]
mod tests {
    use super::Face;

    #[test]
    fn faces_serialize_by_kebab_name_and_default_to_mono() {
        assert_eq!(
            serde_json::to_value(Face::SerifItalic).unwrap(),
            "serif-italic"
        );
        assert_eq!(
            serde_json::from_value::<Face>("shout".into()).unwrap(),
            Face::Shout
        );
        assert!(Face::default().is_mono());
    }
}

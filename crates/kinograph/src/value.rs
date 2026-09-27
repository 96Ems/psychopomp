//! A labeled finite value in a teaching diagram. Motion is authored through
//! ordinary x/y/opacity/emphasis channels, not a second snapshot or graph system.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub const VALUE_TOKEN_RECIPE: &str = "value-token";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValueTokenPlan {
    pub label: String,
    #[serde(default)]
    pub detail: String,
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub font_size: f32,
    pub accent: [u8; 3],
}

impl ValueTokenPlan {
    pub fn validate(&self) -> Result<()> {
        if self.label.is_empty()
            || self.label.contains(['\n', '\r'])
            || self.detail.contains(['\n', '\r'])
        {
            bail!("value token needs a nonempty single-line label and single-line detail");
        }
        if self.center.iter().any(|v| !v.is_finite())
            || self
                .size
                .iter()
                .any(|v| !v.is_finite() || *v < 32. || *v > 1920.)
            || !self.font_size.is_finite()
            || !(8.0..=128.0).contains(&self.font_size)
        {
            bail!(
                "value token needs finite coordinates, dimensions in [32, 1920], and font size in [8, 128]"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_finite_token_geometry_and_immutable_text() {
        let token = ValueTokenPlan {
            label: "true".into(),
            detail: String::new(),
            center: [960., 540.],
            size: [200., 100.],
            font_size: 36.,
            accent: [250, 145, 80],
        };
        token.validate().unwrap();
        for invalid in [
            ValueTokenPlan {
                size: [0., 100.],
                ..token.clone()
            },
            ValueTokenPlan {
                center: [f32::NAN, 0.],
                ..token.clone()
            },
            ValueTokenPlan {
                font_size: f32::INFINITY,
                ..token.clone()
            },
            ValueTokenPlan {
                label: "".into(),
                ..token.clone()
            },
            ValueTokenPlan {
                detail: "two\nlines".into(),
                ..token.clone()
            },
        ] {
            assert!(invalid.validate().is_err());
        }
        serde_json::from_str::<ValueTokenPlan>(&serde_json::to_string(&token).unwrap())
            .unwrap()
            .validate()
            .unwrap();
    }
}

//! Only native appearance is persisted; never clocks, steps, or authored plans.
use crate::render::Theme;
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .map(|p| p.join("psychopomp/preferences.json"))
}

fn document(path: &Path) -> Result<serde_json::Value> {
    match fs::read(path) {
        Ok(bytes) => {
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            anyhow::ensure!(value.is_object(), "preferences must be a JSON object");
            Ok(value)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(error) => Err(error.into()),
    }
}
pub(super) fn load(path: &Path) -> Result<Theme> {
    let value = document(path)?;
    value.get("theme").map_or(Ok(Theme::default()), |t| {
        Ok(serde_json::from_value(t.clone())?)
    })
}
pub(super) fn save(path: &Path, theme: Theme) -> Result<()> {
    let mut value = document(path).context("read appearance preferences before saving")?;
    value["theme"] = serde_json::to_value(theme)?;
    fs::create_dir_all(path.parent().context("preferences directory")?)?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temporary, serde_json::to_vec_pretty(&value)?)?;
    fs::rename(&temporary, path).context("replace appearance preferences")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_theme_survives_restart_without_discarding_other_preferences() {
        let path = std::env::temp_dir().join(format!(
            "psychopomp-preferences-{}.json",
            std::process::id()
        ));
        fs::write(&path, r#"{"other":42}"#).unwrap();
        for theme in Theme::ALL {
            save(&path, theme).unwrap();
            assert_eq!(load(&path).unwrap(), theme);
        }
        assert_eq!(document(&path).unwrap()["other"], 42);
        fs::write(&path, "invalid").unwrap();
        assert!(load(&path).is_err());
        assert!(save(&path, Theme::Black).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid");
        fs::remove_file(path).unwrap();
    }
}

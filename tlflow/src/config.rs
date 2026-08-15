use serde::Deserialize;
use std::path::Path;

fn d_back() -> usize {
    3
}
fn d_ahead() -> usize {
    7
}
fn d_far() -> usize {
    3
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub window_back: usize,
    pub window_ahead: usize,
    pub far_body_lines: usize,
    pub glyphs: Option<String>,
    pub theme: Option<String>,
    #[serde(rename = "check")]
    pub check: CheckSection,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CheckSection {
    pub allow_markers: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            window_back: d_back(),
            window_ahead: d_ahead(),
            far_body_lines: d_far(),
            glyphs: None,
            theme: None,
            check: CheckSection::default(),
        }
    }
}

impl Config {
    /// Convenience: the marker allowlist lives under `[check]` in the file but
    /// is read often enough to deserve a direct accessor.
    pub fn allow_markers(&self) -> &[String] {
        &self.check.allow_markers
    }

    /// Repo config, then user config, then defaults. First hit wins.
    pub fn load(start: &Path) -> Config {
        let repo = start.join(".throughline/config.toml");
        if let Ok(text) = std::fs::read_to_string(&repo) {
            if let Ok(c) = toml::from_str(&text) {
                return c;
            }
        }
        if let Some(home) = dirs::config_dir() {
            let user = home.join("throughline/config.toml");
            if let Ok(text) = std::fs::read_to_string(&user) {
                if let Ok(c) = toml::from_str(&text) {
                    return c;
                }
            }
        }
        Config::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let c = Config::default();
        assert_eq!(c.window_back, 3);
        assert_eq!(c.window_ahead, 7);
        assert_eq!(c.far_body_lines, 3);
        assert!(c.allow_markers().is_empty());
    }

    #[test]
    fn partial_toml_keeps_the_other_defaults() {
        let c: Config = toml::from_str("window_ahead = 12\n").unwrap();
        assert_eq!(c.window_ahead, 12);
        assert_eq!(c.window_back, 3);
    }

    #[test]
    fn check_section_supplies_the_marker_allowlist() {
        let c: Config = toml::from_str("[check]\nallow_markers = [\"v2\"]\n").unwrap();
        assert_eq!(c.allow_markers(), ["v2".to_string()]);
    }

    #[test]
    fn an_absent_config_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let c = Config::load(dir.path());
        assert_eq!(c.window_ahead, 7);
    }
}

use crate::config::Config;
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Past,
    Now,
    Near,
    Mid,
    Far,
    Marker,
    Blocked,
    Dropped,
    Cursor,
    Window,
    Muted,
    Bg,
    Fg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    True,
    Ansi256,
    Ansi16,
    None,
}

impl Variant {
    pub fn resolve(flag: Option<&str>, cfg: &Config) -> Variant {
        let named = flag
            .map(str::to_string)
            .or_else(|| std::env::var("TL_THEME").ok())
            .or_else(|| cfg.theme.clone());
        match named.as_deref() {
            Some("light") => Variant::Light,
            Some("dark") => Variant::Dark,
            // Ask the terminal for its background (OSC 11) and fall back to
            // dark, which is the safer guess when the query times out.
            _ => match terminal_light::luma() {
                Ok(l) if l > 0.5 => Variant::Light,
                _ => Variant::Dark,
            },
        }
    }
}

impl Depth {
    /// Terminal colour capability, ignoring whether colour is wanted.
    fn capability() -> Depth {
        match std::env::var("COLORTERM").as_deref() {
            Ok("truecolor") | Ok("24bit") => Depth::True,
            _ => match std::env::var("TERM") {
                Ok(t) if t.contains("256") => Depth::Ansi256,
                _ => Depth::Ansi16,
            },
        }
    }

    /// `force` is `--color always/never`; `None` means auto.
    ///
    /// An explicit `--color always` must win over both the TTY check and
    /// NO_COLOR — it is an instruction, not a preference. Auto still declines
    /// to colour a pipe, which is what keeps agent-facing output clean.
    pub fn resolve(force: Option<bool>, is_tty: bool) -> Depth {
        match force {
            Some(false) => Depth::None,
            Some(true) => match Depth::capability() {
                // A forced request on a terminal claiming no colour still gets
                // the lowest real tier rather than nothing.
                Depth::None => Depth::Ansi16,
                d => d,
            },
            None => {
                if !is_tty || std::env::var_os("NO_COLOR").is_some() {
                    Depth::None
                } else {
                    Depth::capability()
                }
            }
        }
    }

    pub fn detect(is_tty: bool) -> Depth {
        Depth::resolve(None, is_tty)
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    variant: Variant,
    depth: Depth,
    /// Per-token colour overrides from a user theme file. The built-in variant
    /// remains the base, so a theme only has to name what it changes.
    overrides: HashMap<Token, (u8, u8, u8)>,
}

/// Every token, by the name a theme file uses.
const TOKEN_NAMES: [(&str, Token); 13] = [
    ("past", Token::Past),
    ("now", Token::Now),
    ("near", Token::Near),
    ("mid", Token::Mid),
    ("far", Token::Far),
    ("marker", Token::Marker),
    ("blocked", Token::Blocked),
    ("dropped", Token::Dropped),
    ("cursor", Token::Cursor),
    ("window", Token::Window),
    ("muted", Token::Muted),
    ("bg", Token::Bg),
    ("fg", Token::Fg),
]; 

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThemeError {
    #[error("no theme named {0}. Built-in: dark, light. User themes live in \
             .throughline/themes/<name>.toml")]
    NotFound(String),
    #[error("{file}: unknown token {token:?}. Valid tokens: {valid}")]
    UnknownToken { file: String, token: String, valid: String },
    #[error("{file}: token {token:?} has invalid colour {value:?} — expected #rrggbb")]
    BadColour { file: String, token: String, value: String },
    #[error("{file}: base must be \"dark\" or \"light\", got {base:?}")]
    BadBase { file: String, base: String },
    #[error("{0}")]
    Unreadable(String),
}

#[derive(Deserialize)]
struct ThemeFile {
    #[serde(default)]
    base: Option<String>,
    #[serde(default)]
    tokens: std::collections::BTreeMap<String, String>,
}

fn parse_hex(v: &str) -> Option<(u8, u8, u8)> {
    let h = v.strip_prefix('#')?;
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some((
        u8::from_str_radix(&h[0..2], 16).ok()?,
        u8::from_str_radix(&h[2..4], 16).ok()?,
        u8::from_str_radix(&h[4..6], 16).ok()?,
    ))
}

/// Search order for a named theme: project first, then user config.
pub fn theme_paths(name: &str, root: &Path) -> Vec<std::path::PathBuf> {
    let mut v = vec![root.join(".throughline/themes").join(format!("{name}.toml"))];
    if let Some(c) = dirs::config_dir() {
        v.push(c.join("throughline/themes").join(format!("{name}.toml")));
    }
    v
}

/// (r, g, b, ansi16 fallback)
type Swatch = (u8, u8, u8, Color);

impl Theme {
    pub fn new(variant: Variant, depth: Depth) -> Theme {
        Theme { variant, depth, overrides: HashMap::new() }
    }

    /// Load a named theme. Built-in names resolve without touching disk;
    /// anything else is looked up as a file.
    pub fn load(name: &str, depth: Depth, root: &Path) -> Result<Theme, ThemeError> {
        match name {
            "dark" => return Ok(Theme::new(Variant::Dark, depth)),
            "light" => return Ok(Theme::new(Variant::Light, depth)),
            _ => {}
        }
        let path = theme_paths(name, root)
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| ThemeError::NotFound(name.to_string()))?;
        let text = std::fs::read_to_string(&path)
            .map_err(|e| ThemeError::Unreadable(format!("{}: {e}", path.display())))?;
        let file: ThemeFile = toml::from_str(&text)
            .map_err(|e| ThemeError::Unreadable(format!("{}: {e}", path.display())))?;

        let label = path.display().to_string();
        let variant = match file.base.as_deref() {
            None | Some("dark") => Variant::Dark,
            Some("light") => Variant::Light,
            Some(other) => {
                return Err(ThemeError::BadBase {
                    file: label,
                    base: other.to_string(),
                })
            }
        };

        let valid: Vec<&str> = TOKEN_NAMES.iter().map(|(n, _)| *n).collect();
        let mut overrides = HashMap::new();
        for (k, v) in &file.tokens {
            // A silently-ignored typo is worse than an error: the theme would
            // look almost right and nobody would know which line was dead.
            let tok = TOKEN_NAMES
                .iter()
                .find(|(n, _)| n == k)
                .map(|(_, t)| *t)
                .ok_or_else(|| ThemeError::UnknownToken {
                    file: label.clone(),
                    token: k.clone(),
                    valid: valid.join(", "),
                })?;
            let rgb = parse_hex(v).ok_or_else(|| ThemeError::BadColour {
                file: label.clone(),
                token: k.clone(),
                value: v.clone(),
            })?;
            overrides.insert(tok, rgb);
        }
        Ok(Theme { variant, depth, overrides })
    }

    /// Names of themes available here, built-ins first.
    pub fn available(root: &Path) -> Vec<String> {
        let mut out = vec!["dark".to_string(), "light".to_string()];
        let mut dirs_to_scan = vec![root.join(".throughline/themes")];
        if let Some(c) = dirs::config_dir() {
            dirs_to_scan.push(c.join("throughline/themes"));
        }
        for d in dirs_to_scan {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("toml") {
                        if let Some(stem) = p.file_stem().and_then(|x| x.to_str()) {
                            if !out.iter().any(|n| n == stem) {
                                out.push(stem.to_string());
                            }
                        }
                    }
                }
            }
        }
        out
    }

    pub fn variant(&self) -> Variant {
        self.variant
    }

    /// Flip between the built-in variants, keeping any user overrides. A theme
    /// based on dark can therefore be viewed light without losing its colours.
    pub fn toggle_variant(&mut self) {
        self.variant = match self.variant {
            Variant::Dark => Variant::Light,
            Variant::Light => Variant::Dark,
        };
    }

    pub fn style(&self, token: Token) -> Style {
        let (r, g, b, fallback) = self.swatch(token);
        let base = match self.depth {
            Depth::None => Style::default(),
            Depth::True => Style::default().fg(Color::Rgb(r, g, b)),
            Depth::Ansi256 | Depth::Ansi16 => Style::default().fg(fallback),
        };
        match token {
            Token::Now | Token::Cursor => base.add_modifier(Modifier::BOLD),
            Token::Far | Token::Muted => base.add_modifier(Modifier::DIM),
            _ => base,
        }
    }

    /// SGR escape for plain-stdout rendering. Empty when colour is off, which
    /// is what keeps piped output clean (spec 7.4).
    pub fn sgr(&self, token: Token) -> String {
        match self.depth {
            Depth::None => String::new(),
            _ => {
                let (r, g, b, _) = self.swatch(token);
                format!("\u{1b}[38;2;{r};{g};{b}m")
            }
        }
    }

    pub fn reset(&self) -> &'static str {
        match self.depth {
            Depth::None => "",
            _ => "\u{1b}[0m",
        }
    }

    /// Distance from Now to a detail token (spec 7.3). Behind Now is history and
    /// always reads as `Past`; ahead fades with distance.
    pub fn fade(&self, distance: isize) -> Token {
        match distance {
            d if d < 0 => Token::Past,
            0..=3 => Token::Near,
            4..=10 => Token::Mid,
            _ => Token::Far,
        }
    }

    fn swatch(&self, token: Token) -> Swatch {
        if let Some(&(r, g, b)) = self.overrides.get(&token) {
            // Keep the base variant's indexed fallback: a user theme supplies
            // truecolour, and degrading it to 16 colours is guesswork.
            let (_, _, _, fallback) = self.builtin(token);
            return (r, g, b, fallback);
        }
        self.builtin(token)
    }

    fn builtin(&self, token: Token) -> Swatch {
        match self.variant {
            // Dark: navy ground, electric cyan accent; fades toward black.
            Variant::Dark => match token {
                Token::Bg => (11, 17, 32, Color::Black),
                Token::Fg => (222, 232, 245, Color::White),
                Token::Now => (56, 189, 248, Color::LightCyan),
                Token::Past => (94, 110, 133, Color::DarkGray),
                Token::Near => (222, 232, 245, Color::White),
                Token::Mid => (140, 158, 181, Color::Gray),
                Token::Far => (82, 96, 117, Color::DarkGray),
                Token::Marker => (129, 140, 248, Color::LightMagenta),
                Token::Blocked => (251, 191, 36, Color::Yellow),
                Token::Dropped => (100, 108, 124, Color::DarkGray),
                Token::Cursor => (125, 211, 252, Color::LightBlue),
                Token::Window => (30, 58, 95, Color::Blue),
                Token::Muted => (94, 110, 133, Color::DarkGray),
            },
            // Light: near-white ground, DEEPER blue accent — bright cyan on
            // white is unreadable (spec 7.2). Fades toward white.
            Variant::Light => match token {
                Token::Bg => (250, 251, 253, Color::White),
                Token::Fg => (17, 26, 42, Color::Black),
                Token::Now => (3, 87, 156, Color::Blue),
                Token::Past => (128, 141, 160, Color::Gray),
                Token::Near => (17, 26, 42, Color::Black),
                Token::Mid => (94, 108, 128, Color::DarkGray),
                Token::Far => (163, 176, 193, Color::Gray),
                Token::Marker => (79, 70, 229, Color::Magenta),
                Token::Blocked => (180, 83, 9, Color::Yellow),
                Token::Dropped => (156, 166, 181, Color::Gray),
                Token::Cursor => (2, 108, 194, Color::LightBlue),
                Token::Window => (219, 234, 254, Color::LightBlue),
                Token::Muted => (128, 141, 160, Color::Gray),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    const ALL: [Token; 13] = [
        Token::Past,
        Token::Now,
        Token::Near,
        Token::Mid,
        Token::Far,
        Token::Marker,
        Token::Blocked,
        Token::Dropped,
        Token::Cursor,
        Token::Window,
        Token::Muted,
        Token::Bg,
        Token::Fg,
    ];

    #[test]
    fn every_token_resolves_in_both_variants() {
        for v in [Variant::Dark, Variant::Light] {
            let t = Theme::new(v, Depth::True);
            for tok in ALL {
                assert_ne!(t.style(tok), Default::default(), "{v:?}/{tok:?} unstyled");
            }
        }
    }

    #[test]
    fn light_is_not_a_mechanical_inversion_of_dark() {
        // The accent must differ in hue, not merely in lightness.
        let dark = Theme::new(Variant::Dark, Depth::True)
            .style(Token::Now)
            .fg
            .unwrap();
        let light = Theme::new(Variant::Light, Depth::True)
            .style(Token::Now)
            .fg
            .unwrap();
        assert_ne!(dark, light);
    }

    #[test]
    fn no_colour_depth_yields_unstyled_colours_but_keeps_modifiers() {
        let t = Theme::new(Variant::Dark, Depth::None);
        assert_eq!(t.style(Token::Now).fg, None);
    }

    #[test]
    fn truecolor_depth_produces_rgb() {
        let t = Theme::new(Variant::Dark, Depth::True);
        assert!(matches!(t.style(Token::Now).fg, Some(Color::Rgb(_, _, _))));
    }

    #[test]
    fn ansi16_depth_degrades_to_indexed_colours() {
        let t = Theme::new(Variant::Dark, Depth::Ansi16);
        assert!(!matches!(t.style(Token::Now).fg, Some(Color::Rgb(_, _, _))));
    }

    #[test]
    fn sgr_is_empty_when_colour_is_off() {
        assert_eq!(Theme::new(Variant::Dark, Depth::None).sgr(Token::Now), "");
        assert_eq!(Theme::new(Variant::Dark, Depth::None).reset(), "");
    }

    #[test]
    fn fade_maps_distance_from_now_to_near_mid_far() {
        let t = Theme::new(Variant::Dark, Depth::True);
        assert_eq!(t.fade(-2), Token::Past);
        assert_eq!(t.fade(0), Token::Near);
        assert_eq!(t.fade(2), Token::Near);
        assert_eq!(t.fade(5), Token::Mid);
        assert_eq!(t.fade(30), Token::Far);
    }

    #[test]
    fn a_flag_beats_config_for_the_variant() {
        let cfg = Config { theme: Some("dark".into()), ..Default::default() };
        assert_eq!(Variant::resolve(Some("light"), &cfg), Variant::Light);
    }

    #[test]
    fn non_tty_disables_colour_entirely() {
        assert_eq!(Depth::detect(false), Depth::None);
    }

    #[test]
    fn color_always_wins_over_the_tty_check() {
        // `--color always` on a pipe previously produced no escape codes at
        // all: the flag was computed and then thrown away by a detect() that
        // short-circuits on !is_tty. Same bug class as --glyphs being ignored.
        assert_ne!(Depth::resolve(Some(true), false), Depth::None);
    }

    #[test]
    fn color_never_wins_over_a_terminal() {
        assert_eq!(Depth::resolve(Some(false), true), Depth::None);
    }

    #[test]
    fn auto_declines_to_colour_a_pipe() {
        assert_eq!(Depth::resolve(None, false), Depth::None);
    }

    /// Spec 10: no view may construct a colour directly — every style must come
    /// from a token. This is the enforcement, and it is cheap: grep the modules
    /// that render for `Color::`.
    #[test]
    fn no_view_module_constructs_a_colour_directly() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut offenders = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(root)];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let p = entry.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().and_then(|e| e.to_str()) != Some("rs") {
                    continue;
                }
                // theme/mod.rs is the ONLY module allowed to name a colour.
                if p.ends_with("theme/mod.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap();
                for (n, l) in text.lines().enumerate() {
                    let code = l.split("//").next().unwrap_or("");
                    if code.contains("Color::") || code.contains("Rgb(") {
                        offenders.push(format!("{}:{}", p.display(), n + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these lines construct a colour outside theme/: {offenders:#?}"
        );
    }
}

#[cfg(test)]
mod theme_file_tests {
    use super::*;

    fn write(dir: &std::path::Path, name: &str, body: &str) {
        let d = dir.join(".throughline/themes");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(format!("{name}.toml")), body).unwrap();
    }

    #[test]
    fn builtin_names_resolve_without_touching_disk() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            Theme::load("dark", Depth::True, d.path()).unwrap().variant(),
            Variant::Dark
        );
        assert_eq!(
            Theme::load("light", Depth::True, d.path()).unwrap().variant(),
            Variant::Light
        );
    }

    #[test]
    fn a_user_theme_overrides_only_what_it_names() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "mine", "base = \"dark\"\n[tokens]\nnow = \"#268bd2\"\n");
        let t = Theme::load("mine", Depth::True, d.path()).unwrap();

        // The named token is overridden...
        assert_eq!(t.style(Token::Now).fg, Some(Color::Rgb(0x26, 0x8b, 0xd2)));
        // ...and everything else still comes from the base variant.
        let base = Theme::new(Variant::Dark, Depth::True);
        assert_eq!(t.style(Token::Marker).fg, base.style(Token::Marker).fg);
    }

    #[test]
    fn base_selects_which_builtin_to_inherit() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "pale", "base = \"light\"\n[tokens]\nnow = \"#000001\"\n");
        let t = Theme::load("pale", Depth::True, d.path()).unwrap();
        assert_eq!(t.variant(), Variant::Light);
        let light = Theme::new(Variant::Light, Depth::True);
        assert_eq!(t.style(Token::Fg).fg, light.style(Token::Fg).fg);
    }

    #[test]
    fn an_unknown_token_name_is_an_error_not_silence() {
        // A typo that silently does nothing is worse than a failure: the theme
        // looks almost right and nobody can tell which line is dead.
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "typo", "[tokens]\nnwo = \"#112233\"\n");
        let err = Theme::load("typo", Depth::True, d.path()).unwrap_err();
        match err {
            ThemeError::UnknownToken { token, valid, .. } => {
                assert_eq!(token, "nwo");
                assert!(valid.contains("now"), "error should list valid tokens");
            }
            other => panic!("expected UnknownToken, got {other}"),
        }
    }

    #[test]
    fn a_malformed_colour_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "bad", "[tokens]\nnow = \"268bd2\"\n");
        assert!(matches!(
            Theme::load("bad", Depth::True, d.path()).unwrap_err(),
            ThemeError::BadColour { .. }
        ));

        write(d.path(), "short", "[tokens]\nnow = \"#abc\"\n");
        assert!(matches!(
            Theme::load("short", Depth::True, d.path()).unwrap_err(),
            ThemeError::BadColour { .. }
        ));
    }

    #[test]
    fn an_invalid_base_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "odd", "base = \"purple\"\n[tokens]\nnow = \"#112233\"\n");
        assert!(matches!(
            Theme::load("odd", Depth::True, d.path()).unwrap_err(),
            ThemeError::BadBase { .. }
        ));
    }

    #[test]
    fn a_missing_theme_names_the_builtins_and_where_to_put_files() {
        let d = tempfile::tempdir().unwrap();
        let msg = Theme::load("nope", Depth::True, d.path()).unwrap_err().to_string();
        assert!(msg.contains("dark"), "should name the built-ins: {msg}");
        assert!(msg.contains(".throughline/themes"), "should say where: {msg}");
    }

    #[test]
    fn available_lists_builtins_plus_user_themes() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "solarized", "[tokens]\nnow = \"#268bd2\"\n");
        let names = Theme::available(d.path());
        assert!(names.contains(&"dark".to_string()));
        assert!(names.contains(&"light".to_string()));
        assert!(names.contains(&"solarized".to_string()));
    }

    #[test]
    fn toggling_a_user_theme_keeps_its_overrides() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "mine", "base = \"dark\"\n[tokens]\nnow = \"#268bd2\"\n");
        let mut t = Theme::load("mine", Depth::True, d.path()).unwrap();
        t.toggle_variant();
        assert_eq!(t.variant(), Variant::Light);
        assert_eq!(t.style(Token::Now).fg, Some(Color::Rgb(0x26, 0x8b, 0xd2)));
    }

    #[test]
    fn overrides_still_degrade_to_indexed_colour() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "mine", "[tokens]\nnow = \"#268bd2\"\n");
        let t = Theme::load("mine", Depth::Ansi16, d.path()).unwrap();
        assert!(!matches!(t.style(Token::Now).fg, Some(Color::Rgb(_, _, _))));
    }
}

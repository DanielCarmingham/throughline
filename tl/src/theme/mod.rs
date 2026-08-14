use crate::config::Config;
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn detect(is_tty: bool) -> Depth {
        if !is_tty || std::env::var_os("NO_COLOR").is_some() {
            return Depth::None;
        }
        match std::env::var("COLORTERM").as_deref() {
            Ok("truecolor") | Ok("24bit") => Depth::True,
            _ => match std::env::var("TERM") {
                Ok(t) if t.contains("256") => Depth::Ansi256,
                _ => Depth::Ansi16,
            },
        }
    }
}

pub struct Theme {
    variant: Variant,
    depth: Depth,
}

/// (r, g, b, ansi16 fallback)
type Swatch = (u8, u8, u8, Color);

impl Theme {
    pub fn new(variant: Variant, depth: Depth) -> Theme {
        Theme { variant, depth }
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
        let mut cfg = Config::default();
        cfg.theme = Some("dark".into());
        assert_eq!(Variant::resolve(Some("light"), &cfg), Variant::Light);
    }

    #[test]
    fn non_tty_disables_colour_entirely() {
        assert_eq!(Depth::detect(false), Depth::None);
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

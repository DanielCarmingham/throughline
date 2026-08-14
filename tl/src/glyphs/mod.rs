use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Done,
    Open,
    Active,
    Dropped,
    Blocked,
    Marker,
    Now,
    Arrow,
    Children,
    Sharpened,
    Coarse,
    Cycle,
    History,
    ZoomOut,
    Search,
    WindowLeft,
    WindowRight,
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    NerdFont,
    Unicode,
    Ascii,
}

impl Mode {
    /// flag > env > config > detection. Non-TTY always wins: an agent piping
    /// output must never receive glyphs it cannot render (spec 7.4).
    pub fn resolve(flag: Option<&str>, cfg: &Config, is_tty: bool) -> Mode {
        if !is_tty {
            return Mode::Ascii;
        }
        let named = flag
            .map(str::to_string)
            .or_else(|| std::env::var("TL_GLYPHS").ok())
            .or_else(|| cfg.glyphs.clone());
        match named.as_deref() {
            Some("nerdfont") => Mode::NerdFont,
            Some("ascii") => Mode::Ascii,
            Some("unicode") => Mode::Unicode,
            // Nerd Font support is not reliably detectable; `tl doctor` asks
            // once and writes the answer to config (spec 7.1).
            _ => Mode::Unicode,
        }
    }
}

pub struct Glyphs {
    mode: Mode,
}

impl Glyphs {
    pub fn for_mode(mode: Mode) -> Glyphs {
        Glyphs { mode }
    }

    pub fn get(&self, role: Role) -> &'static str {
        match self.mode {
            Mode::NerdFont => nerdfont(role),
            Mode::Unicode => unicode(role),
            Mode::Ascii => ascii(role),
        }
    }
}

/// Codicons throughout, for uniform stroke weight (spec 7.1).
fn nerdfont(role: Role) -> &'static str {
    match role {
        Role::Done => "\u{ebb3}",        // cod-pass_filled
        Role::Open => "\u{ebb5}",        // cod-circle_large
        Role::Active => "\u{eba6}",      // cod-play_circle
        Role::Dropped => "\u{eabd}",     // cod-circle_slash
        Role::Blocked => "\u{ea6c}",     // cod-warning
        Role::Marker => "\u{eb20}",      // cod-milestone
        Role::Now => "\u{eb1a}",         // cod-location
        Role::Arrow => "\u{eb70}",       // cod-triangle_right
        Role::Children => "\u{eb17}",    // cod-list_unordered
        Role::Sharpened => "\u{eb26}",   // cod-note
        Role::Coarse => "\u{ea61}",      // cod-lightbulb
        Role::Cycle => "\u{ea77}",       // cod-sync
        Role::History => "\u{ea82}",     // cod-history
        Role::ZoomOut => "\u{eb82}",     // cod-zoom_out
        Role::Search => "\u{ea6d}",      // cod-search
        Role::WindowLeft => "\u{e0b6}",  // ple-left_half_circle_thick
        Role::WindowRight => "\u{e0b4}", // ple-right_half_circle_thick
        Role::Rule => "─",               // deliberately not cod-horizontal_rule
    }
}

fn unicode(role: Role) -> &'static str {
    match role {
        Role::Done => "●",
        Role::Open => "○",
        Role::Active => "◉",
        Role::Dropped => "⊘",
        Role::Blocked => "⚠",
        Role::Marker => "◆",
        Role::Now => "│",
        Role::Arrow => "▶",
        Role::Children => "▾",
        Role::Sharpened => "≡",
        Role::Coarse => "·",
        Role::Cycle => "↻",
        Role::History => "⟲",
        Role::ZoomOut => "⊟",
        Role::Search => "⌕",
        Role::WindowLeft => "┌",
        Role::WindowRight => "┐",
        Role::Rule => "─",
    }
}

fn ascii(role: Role) -> &'static str {
    match role {
        Role::Done => "[x]",
        Role::Open => "[ ]",
        Role::Active => "[>]",
        Role::Dropped => "[-]",
        Role::Blocked => "!",
        Role::Marker => "<>",
        Role::Now => "|",
        Role::Arrow => ">",
        Role::Children => "+",
        Role::Sharpened => "=",
        Role::Coarse => "~",
        Role::Cycle => "@",
        Role::History => "<<",
        Role::ZoomOut => "-",
        Role::Search => "/",
        Role::WindowLeft => "[",
        Role::WindowRight => "]",
        Role::Rule => "-",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Role; 18] = [
        Role::Done,
        Role::Open,
        Role::Active,
        Role::Dropped,
        Role::Blocked,
        Role::Marker,
        Role::Now,
        Role::Arrow,
        Role::Children,
        Role::Sharpened,
        Role::Coarse,
        Role::Cycle,
        Role::History,
        Role::ZoomOut,
        Role::Search,
        Role::WindowLeft,
        Role::WindowRight,
        Role::Rule,
    ];

    #[test]
    fn every_role_has_a_glyph_in_every_mode() {
        for mode in [Mode::NerdFont, Mode::Unicode, Mode::Ascii] {
            let g = Glyphs::for_mode(mode);
            for role in ALL {
                assert!(!g.get(role).is_empty(), "{mode:?} has no glyph for {role:?}");
            }
        }
    }

    #[test]
    fn ascii_mode_is_pure_seven_bit() {
        let g = Glyphs::for_mode(Mode::Ascii);
        for role in ALL {
            assert!(
                g.get(role).is_ascii(),
                "{role:?} is not ascii: {:?}",
                g.get(role)
            );
        }
    }

    #[test]
    fn nerdfont_codepoints_match_the_spec() {
        let g = Glyphs::for_mode(Mode::NerdFont);
        assert_eq!(g.get(Role::Done), "\u{ebb3}"); // cod-pass_filled
        assert_eq!(g.get(Role::Open), "\u{ebb5}"); // cod-circle_large
        assert_eq!(g.get(Role::Now), "\u{eb1a}"); // cod-location
        assert_eq!(g.get(Role::Marker), "\u{eb20}"); // cod-milestone
        assert_eq!(g.get(Role::WindowLeft), "\u{e0b6}");
        assert_eq!(g.get(Role::WindowRight), "\u{e0b4}");
    }

    #[test]
    fn the_ribbon_rule_stays_unicode_in_nerdfont_mode() {
        // cod-horizontal_rule does not tile; spec 7.1 pins this deliberately.
        assert_eq!(Glyphs::for_mode(Mode::NerdFont).get(Role::Rule), "─");
    }

    #[test]
    fn a_flag_beats_config() {
        let mut cfg = Config::default();
        cfg.glyphs = Some("unicode".into());
        assert_eq!(Mode::resolve(Some("ascii"), &cfg, true), Mode::Ascii);
    }

    #[test]
    fn config_is_used_when_no_flag_is_given() {
        let mut cfg = Config::default();
        cfg.glyphs = Some("nerdfont".into());
        assert_eq!(Mode::resolve(None, &cfg, true), Mode::NerdFont);
    }

    #[test]
    fn non_tty_forces_ascii_regardless_of_config() {
        let mut cfg = Config::default();
        cfg.glyphs = Some("nerdfont".into());
        assert_eq!(Mode::resolve(None, &cfg, false), Mode::Ascii);
    }

    #[test]
    fn unicode_is_the_fallback_on_a_tty_with_no_preference() {
        assert_eq!(Mode::resolve(None, &Config::default(), true), Mode::Unicode);
    }
}

use crate::glyphs::{Glyphs, Role};
use crate::model::{Entry, ItemState, Line};
use crate::theme::Token;
use crate::view::{self, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub token: Token,
}

pub fn plain(segments: &[Segment]) -> String {
    segments.iter().map(|s| s.text.as_str()).collect()
}

fn glyph_for(entry: &Entry, behind_now: bool, g: &Glyphs) -> String {
    match entry {
        Entry::Now => g.get(Role::Now).to_string(),
        Entry::Marker(_) => g.get(Role::Marker).to_string(),
        Entry::Item(item) => {
            let role = match (&item.state, behind_now) {
                (ItemState::Dropped(_), _) => Role::Dropped,
                (ItemState::Blocked(_), _) => Role::Blocked,
                (ItemState::Active, _) => Role::Active,
                (_, true) => Role::Done,
                (_, false) => Role::Open,
            };
            g.get(role).to_string()
        }
    }
}

fn token_for(line: &Line, index: usize, entry: &Entry) -> Token {
    match entry {
        Entry::Now => Token::Now,
        Entry::Marker(_) => Token::Marker,
        Entry::Item(item) => match &item.state {
            ItemState::Dropped(_) => Token::Dropped,
            ItemState::Blocked(_) => Token::Blocked,
            _ => {
                let d = view::distance_from_now(line, index);
                if d < 0 {
                    Token::Past
                } else if d <= 3 {
                    Token::Near
                } else if d <= 10 {
                    Token::Mid
                } else {
                    Token::Far
                }
            }
        },
    }
}

/// Build the whole-line ribbon. `window` is bracketed; when the line is
/// wider than `width`, entries are dropped from the ends inward so that Now
/// always survives.
pub fn build(line: &Line, window: Span, g: &Glyphs, width: usize) -> Vec<Segment> {
    let now = line.now_index();
    let mut visible: Vec<usize> = (0..line.entries.len()).collect();
    let mut elided_left = false;
    let mut elided_right = false;

    let assemble = |visible: &[usize], left: bool, right: bool| -> Vec<Segment> {
        let mut out = Vec::new();
        if left {
            out.push(Segment {
                text: "...".into(),
                token: Token::Muted,
            });
        }
        for (n, &i) in visible.iter().enumerate() {
            if i == window.start {
                out.push(Segment {
                    text: g.get(Role::WindowLeft).to_string(),
                    token: Token::Window,
                });
            }
            out.push(Segment {
                text: glyph_for(&line.entries[i], i < now, g),
                token: token_for(line, i, &line.entries[i]),
            });
            if i + 1 == window.end {
                out.push(Segment {
                    text: g.get(Role::WindowRight).to_string(),
                    token: Token::Window,
                });
            }
            if n + 1 < visible.len() {
                out.push(Segment {
                    text: g.get(Role::Rule).to_string(),
                    token: Token::Muted,
                });
            }
        }
        if right {
            out.push(Segment {
                text: "...".into(),
                token: Token::Muted,
            });
        }
        out.push(Segment {
            text: g.get(Role::Arrow).to_string(),
            token: Token::Muted,
        });
        out
    };

    // Measure what will actually be drawn rather than estimating it. The
    // elision markers, window brackets and arrow all cost width, and a
    // constant fudge factor for them drifts the moment a glyph set changes.
    loop {
        let segs = assemble(&visible, elided_left, elided_right);
        if plain(&segs).chars().count() <= width || visible.len() <= 1 {
            return segs;
        }
        let now_pos = visible.iter().position(|&i| i == now).unwrap_or(0);
        // Trim from whichever side is longer, so Now stays roughly centred.
        if now_pos >= visible.len() - now_pos - 1 {
            visible.remove(0);
            elided_left = true;
        } else {
            visible.pop();
            elided_right = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;
    use crate::glyphs::Mode;

    fn line(past: usize, future: usize) -> Line {
        let mut src = String::from("# T\n\n");
        for n in 0..past {
            src.push_str(&format!("- [x] p{n}  ^p{n}\n"));
        }
        src.push_str("\n── NOW ──\n\n");
        for n in 0..future {
            src.push_str(&format!("- [ ] f{n}  ^f{n}\n"));
        }
        parse(&src).unwrap()
    }

    fn render(l: &Line, width: usize) -> String {
        let w = view::window(l, &Config::default());
        plain(&build(l, w, &Glyphs::for_mode(Mode::Ascii), width))
    }

    #[test]
    fn done_work_uses_the_done_glyph_and_future_work_the_open_one() {
        let out = render(&line(2, 2), 80);
        assert_eq!(out.matches("[x]").count(), 2);
        assert_eq!(out.matches("[ ]").count(), 2);
    }

    #[test]
    fn now_appears_exactly_once_between_past_and_future() {
        let out = render(&line(3, 3), 80);
        assert_eq!(out.matches('|').count(), 1);
        let now = out.find('|').unwrap();
        assert!(out[..now].contains("[x]"));
        assert!(out[now..].contains("[ ]"));
    }

    #[test]
    fn the_window_is_drawn_as_a_bracket_around_the_focus() {
        let out = render(&line(6, 12), 100);
        let open = out.find('[').unwrap();
        let close = out.rfind(']').unwrap();
        assert!(open < close);
    }

    #[test]
    fn markers_render_with_the_marker_glyph() {
        let l = parse("# T\n\n── NOW ──\n\n◆ v1 ◆\n\n- [ ] a  ^aaa\n").unwrap();
        assert!(render(&l, 80).contains("<>"));
    }

    #[test]
    fn dropped_and_blocked_items_get_their_own_glyphs() {
        let l = parse(
            "# T\n\n- [-] a  ^aaa  @dropped(no)\n\n── NOW ──\n\n- [ ] b  ^bbb  @blocked(keys)\n",
        )
        .unwrap();
        let out = render(&l, 80);
        assert!(out.contains("[-]"));
        assert!(out.contains('!'));
    }

    #[test]
    fn a_line_wider_than_the_terminal_is_elided_around_now() {
        let out = render(&line(40, 40), 40);
        assert!(out.chars().count() <= 40, "ribbon overflowed: {out:?}");
        assert!(out.contains("..."), "expected elision markers");
        assert!(out.contains('|'), "Now must survive elision");
    }

    #[test]
    fn tokens_encode_the_progressive_fade() {
        let l = line(2, 20);
        let w = view::window(&l, &Config::default());
        let segs = build(&l, w, &Glyphs::for_mode(Mode::Ascii), 200);
        assert!(segs.iter().any(|s| s.token == Token::Past));
        assert!(segs.iter().any(|s| s.token == Token::Near));
        assert!(segs.iter().any(|s| s.token == Token::Far));
    }

    #[test]
    fn snapshot_of_a_typical_ribbon() {
        insta::assert_snapshot!(render(&line(5, 9), 72));
    }
}

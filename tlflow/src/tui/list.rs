use crate::glyphs::{Glyphs, Role};
use crate::model::{Entry, ItemState};
use crate::theme::Token;
use crate::tui::app::App;
use crate::tui::ribbon::Segment;
use crate::view;

/// One row per entry in the window, with readable titles and bodies — the
/// zoom-in half of the two-level view.
pub fn build(app: &App, g: &Glyphs) -> Vec<Vec<Segment>> {
    let window = view::window(&app.line, &app.cfg);
    let now = app.line.now_index();
    let mut rows = Vec::new();

    for i in window.start..window.end.min(app.line.entries.len()) {
        let mut row = Vec::new();
        let is_cursor = i == app.cursor;
        row.push(Segment {
            text: if is_cursor { "> ".into() } else { "  ".into() },
            token: if is_cursor { Token::Cursor } else { Token::Muted },
        });

        match &app.line.entries[i] {
            Entry::Now => row.push(Segment {
                text: format!("{} NOW", g.get(Role::Now)),
                token: Token::Now,
            }),
            Entry::Marker(m) => row.push(Segment {
                text: format!("{} {}", g.get(Role::Marker), m.label),
                token: Token::Marker,
            }),
            Entry::Item(item) => {
                let role = match (&item.state, i < now) {
                    (ItemState::Dropped(_), _) => Role::Dropped,
                    (ItemState::Blocked(_), _) => Role::Blocked,
                    (ItemState::Active, _) => Role::Active,
                    (_, true) => Role::Done,
                    (_, false) => Role::Open,
                };
                let token = if is_cursor {
                    Token::Cursor
                } else {
                    match &item.state {
                        ItemState::Dropped(_) => Token::Dropped,
                        ItemState::Blocked(_) => Token::Blocked,
                        _ => {
                            let d = view::distance_from_now(&app.line, i);
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
                    }
                };
                row.push(Segment {
                    text: format!("{} {}  ^{}", g.get(role), item.title, item.id.0),
                    token,
                });
                // Bodies become their own muted rows beneath the title.
                for d in &item.description {
                    rows.push(std::mem::take(&mut row));
                    row = vec![Segment {
                        text: format!("      {d}"),
                        token: Token::Muted,
                    }];
                }
            }
        }
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;
    use crate::glyphs::Mode;
    use crate::tui::app::App;
    use crate::tui::ribbon::plain;

    fn app() -> App {
        let l = parse(
            "# T\n\n- [x] a  ^aaa\n\n── NOW ──\n\n- [ ] b  ^bbb\n      why b matters\n- [ ] c  ^ccc\n",
        )
        .unwrap();
        App::new(l, Config::default())
    }

    #[test]
    fn each_window_entry_produces_at_least_one_row() {
        let rows = build(&app(), &Glyphs::for_mode(Mode::Ascii));
        // 4 entries (a, NOW, b, c) plus one body row for b.
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn rows_carry_readable_titles_unlike_the_ribbon() {
        let rows = build(&app(), &Glyphs::for_mode(Mode::Ascii));
        let text: String = rows.iter().map(|r| plain(r)).collect();
        assert!(text.contains("why b matters"));
        assert!(text.contains("^bbb"));
    }

    #[test]
    fn the_cursor_row_is_marked() {
        let mut a = app();
        a.cursor = 2;
        let rows = build(&a, &Glyphs::for_mode(Mode::Ascii));
        assert!(rows.iter().any(|r| r.iter().any(|s| s.token == Token::Cursor)));
    }

    #[test]
    fn snapshot_of_the_window_list() {
        let rows = build(&app(), &Glyphs::for_mode(Mode::Ascii));
        let text: String = rows.iter().map(|r| format!("{}\n", plain(r))).collect();
        insta::assert_snapshot!(text);
    }
}

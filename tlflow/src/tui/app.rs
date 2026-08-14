use crate::config::Config;
use crate::model::{Entry, Id, Item, Line, Marker, Position, Ref};
use crate::theme::{Depth, Theme, Variant};
use crossterm::event::KeyCode;

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Prompt {
    Add,
    Sharpen,
    Mark,
    Search,
}

pub struct App {
    pub line: Line,
    /// A VIEW position. Now is data; the cursor is where you are looking.
    pub cursor: usize,
    pub cfg: Config,
    pub dirty: bool,
    pub theme: Theme,
    pub prompt: Option<Prompt>,
    pub buffer: String,
    pub help: bool,
}

impl App {
    pub fn new(line: Line, cfg: Config) -> App {
        let cursor = line.now_index();
        App {
            line,
            cursor,
            cfg,
            dirty: false,
            theme: Theme::new(Variant::Dark, Depth::True),
            prompt: None,
            buffer: String::new(),
            help: false,
        }
    }

    fn cursor_id(&self) -> Option<Id> {
        match &self.line.entries[self.cursor] {
            Entry::Item(i) => Some(i.id.clone()),
            _ => None,
        }
    }

    pub fn on_key(&mut self, key: KeyCode) -> Action {
        let last = self.line.entries.len().saturating_sub(1);
        match key {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Char('j') => self.cursor = (self.cursor + 1).min(last),
            KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('g') => self.cursor = 0,
            KeyCode::Char('G') => self.cursor = last,
            KeyCode::Char('n') => self.cursor = self.line.now_index(),
            KeyCode::Char(']') => self.cfg.window_ahead += 1,
            KeyCode::Char('[') => {
                self.cfg.window_ahead = self.cfg.window_ahead.saturating_sub(1).max(1)
            }
            KeyCode::Char('t') => self.theme.toggle_variant(),
            KeyCode::Char('J') if self.cursor < last => {
                let here = self.cursor;
                let what = ref_at(&self.line, here);
                let below = ref_at(&self.line, here + 1);
                if let (Some(w), Some(b)) = (what, below) {
                    if self.line.move_entry(&w, &Position::After(b)).is_ok() {
                        self.cursor = here + 1;
                        self.dirty = true;
                    }
                }
            }
            KeyCode::Char('K') if self.cursor > 0 => {
                let here = self.cursor;
                let what = ref_at(&self.line, here);
                let above = ref_at(&self.line, here - 1);
                if let (Some(w), Some(a)) = (what, above) {
                    if self.line.move_entry(&w, &Position::Before(a)).is_ok() {
                        self.cursor = here - 1;
                        self.dirty = true;
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(id) = self.cursor_id() {
                    if self.line.advance(Some(&Ref::Id(id))).is_ok() {
                        self.dirty = true;
                    }
                }
            }
            KeyCode::Char('d') => {
                if let Some(id) = self.cursor_id() {
                    if self
                        .line
                        .drop_item(&id, "dropped in the TUI".into())
                        .is_ok()
                    {
                        self.dirty = true;
                    }
                }
            }
            KeyCode::Char('a') => {
                self.prompt = Some(Prompt::Add);
                self.buffer.clear();
            }
            KeyCode::Char('s') => {
                self.prompt = Some(Prompt::Sharpen);
                self.buffer = self
                    .cursor_id()
                    .and_then(|id| self.line.item(&id).map(|i| i.description.join(" ")))
                    .unwrap_or_default();
            }
            KeyCode::Char('m') => {
                self.prompt = Some(Prompt::Mark);
                self.buffer.clear();
            }
            KeyCode::Char('/') => {
                self.prompt = Some(Prompt::Search);
                self.buffer.clear();
            }
            KeyCode::Char('?') => self.help = !self.help,
            _ => {}
        }
        Action::None
    }

    /// Apply whatever the open prompt was collecting. Called on Enter.
    pub fn commit_prompt(&mut self) {
        let text = std::mem::take(&mut self.buffer);
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let anchor = ref_at(&self.line, self.cursor);
        match (prompt, anchor) {
            (Prompt::Add, Some(a)) => {
                let id = crate::cli::fresh_id(&self.line, &text);
                if self
                    .line
                    .insert(Entry::Item(Item::new(id, text)), &Position::After(a))
                    .is_ok()
                {
                    self.cursor += 1;
                    self.dirty = true;
                }
            }
            (Prompt::Mark, Some(a)) => {
                if self
                    .line
                    .insert(Entry::Marker(Marker { label: text }), &Position::After(a))
                    .is_ok()
                {
                    self.dirty = true;
                }
            }
            (Prompt::Sharpen, _) => {
                if let Entry::Item(item) = &mut self.line.entries[self.cursor] {
                    item.description = text.lines().map(str::to_string).collect();
                    self.dirty = true;
                }
            }
            (Prompt::Search, _) => {
                let needle = text.to_lowercase();
                if let Some(found) = self.line.entries.iter().position(|e| match e {
                    Entry::Item(i) => i.title.to_lowercase().contains(&needle),
                    Entry::Marker(m) => m.label.to_lowercase().contains(&needle),
                    Entry::Now => false,
                }) {
                    self.cursor = found;
                }
            }
            _ => {}
        }
    }
}

fn ref_at(line: &Line, index: usize) -> Option<Ref> {
    match line.entries.get(index)? {
        Entry::Item(i) => Some(Ref::Id(i.id.clone())),
        Entry::Marker(m) => Some(Ref::Marker(m.label.clone())),
        Entry::Now => Some(Ref::Now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;
    use crate::model::Id;
    use crossterm::event::KeyCode;

    fn app() -> App {
        let l = parse(
            "# T\n\n- [x] a  ^aaa\n\n── NOW ──\n\n- [ ] b  ^bbb\n- [ ] c  ^ccc\n- [ ] d  ^ddd\n",
        )
        .unwrap();
        App::new(l, Config::default())
    }

    #[test]
    fn j_and_k_move_the_cursor_without_writing() {
        let mut a = app();
        let start = a.cursor;
        a.on_key(KeyCode::Char('j'));
        assert_eq!(a.cursor, start + 1);
        a.on_key(KeyCode::Char('k'));
        assert_eq!(a.cursor, start);
        assert!(!a.dirty, "cursor movement must not mark the line dirty");
    }

    #[test]
    fn the_cursor_clamps_at_both_ends() {
        let mut a = app();
        for _ in 0..50 {
            a.on_key(KeyCode::Char('j'));
        }
        assert_eq!(a.cursor, a.line.entries.len() - 1);
        for _ in 0..50 {
            a.on_key(KeyCode::Char('k'));
        }
        assert_eq!(a.cursor, 0);
    }

    #[test]
    fn shift_j_reorders_the_item_under_the_cursor() {
        let mut a = app();
        a.cursor = 2; // ^bbb
        a.on_key(KeyCode::Char('J'));
        assert_eq!(a.cursor, 3);
        let titles: Vec<String> = a.line.items().map(|i| i.title.clone()).collect();
        assert_eq!(titles, ["a", "c", "b", "d"]);
        assert!(a.dirty);
    }

    #[test]
    fn n_returns_the_cursor_to_now() {
        let mut a = app();
        a.cursor = 4;
        a.on_key(KeyCode::Char('n'));
        assert_eq!(a.cursor, a.line.now_index());
    }

    #[test]
    fn space_advances_now_past_the_cursor() {
        let mut a = app();
        a.cursor = 2; // ^bbb
        a.on_key(KeyCode::Char(' '));
        assert!(a.line.is_behind_now(&Id::new("bbb")));
        assert!(a.dirty);
    }

    #[test]
    fn brackets_resize_the_window_without_writing() {
        let mut a = app();
        let before = a.cfg.window_ahead;
        a.on_key(KeyCode::Char(']'));
        assert_eq!(a.cfg.window_ahead, before + 1);
        a.on_key(KeyCode::Char('['));
        assert_eq!(a.cfg.window_ahead, before);
        assert!(!a.dirty);
    }

    #[test]
    fn g_and_shift_g_jump_to_the_ends() {
        let mut a = app();
        a.on_key(KeyCode::Char('G'));
        assert_eq!(a.cursor, a.line.entries.len() - 1);
        a.on_key(KeyCode::Char('g'));
        assert_eq!(a.cursor, 0);
    }

    #[test]
    fn t_toggles_the_theme_variant() {
        let mut a = app();
        let before = a.theme.variant();
        a.on_key(KeyCode::Char('t'));
        assert_ne!(a.theme.variant(), before);
    }

    #[test]
    fn q_requests_quit() {
        let mut a = app();
        assert_eq!(a.on_key(KeyCode::Char('q')), Action::Quit);
    }

    #[test]
    fn d_drops_the_item_under_the_cursor() {
        let mut a = app();
        a.cursor = 2;
        a.on_key(KeyCode::Char('d'));
        assert!(a.line.is_behind_now(&Id::new("bbb")));
    }

    #[test]
    fn a_opens_the_add_prompt_and_enter_inserts_after_the_cursor() {
        let mut a = app();
        a.cursor = 2; // ^bbb
        a.on_key(KeyCode::Char('a'));
        assert_eq!(a.prompt, Some(Prompt::Add));
        a.buffer = "new work".into();
        a.commit_prompt();

        let titles: Vec<String> = a.line.items().map(|i| i.title.clone()).collect();
        assert_eq!(titles, ["a", "b", "new work", "c", "d"]);
        assert!(a.dirty);
    }

    #[test]
    fn s_prefills_the_sharpen_prompt_with_the_existing_body() {
        let mut a = app();
        a.cursor = 2;
        a.on_key(KeyCode::Char('s'));
        assert_eq!(a.prompt, Some(Prompt::Sharpen));
        a.buffer = "why b matters".into();
        a.commit_prompt();
        assert_eq!(
            a.line.item(&Id::new("bbb")).unwrap().description,
            vec!["why b matters".to_string()]
        );
    }

    #[test]
    fn m_places_a_marker_after_the_cursor() {
        let mut a = app();
        a.cursor = 2;
        a.on_key(KeyCode::Char('m'));
        a.buffer = "v0.2".into();
        a.commit_prompt();
        assert!(a
            .line
            .entries
            .iter()
            .any(|e| matches!(e, Entry::Marker(m) if m.label == "v0.2")));
    }

    #[test]
    fn slash_searches_and_moves_the_cursor_without_writing() {
        let mut a = app();
        a.on_key(KeyCode::Char('/'));
        a.buffer = "d".into();
        a.commit_prompt();
        assert_eq!(a.cursor, 4); // ^ddd
        assert!(!a.dirty, "search must not modify the line");
    }

    #[test]
    fn an_empty_prompt_is_a_no_op() {
        let mut a = app();
        let before = a.line.entries.len();
        a.on_key(KeyCode::Char('a'));
        a.commit_prompt();
        assert_eq!(a.line.entries.len(), before);
        assert!(!a.dirty);
    }

    #[test]
    fn question_mark_toggles_help() {
        let mut a = app();
        assert!(!a.help);
        a.on_key(KeyCode::Char('?'));
        assert!(a.help);
        a.on_key(KeyCode::Char('?'));
        assert!(!a.help);
    }
}

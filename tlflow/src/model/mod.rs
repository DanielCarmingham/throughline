pub mod ops;
pub use ops::OpError;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Id(pub String);

impl Id {
    pub fn new(s: impl Into<String>) -> Self {
        Id(s.into())
    }
}

/// Exceptional metadata. There is deliberately no `Done` variant:
/// completion is position (spec 3.1), not state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ItemState {
    Plain,
    Active,
    Blocked(String),
    Dropped(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Child {
    pub title: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub id: Id,
    pub title: String,
    /// Intent, written ahead of Now.
    pub description: Vec<String>,
    /// Outcome, written behind Now.
    pub result: Vec<String>,
    pub children: Vec<Child>,
    pub state: ItemState,
    pub commit: Option<String>,
}

impl Item {
    pub fn new(id: Id, title: impl Into<String>) -> Self {
        Item {
            id,
            title: title.into(),
            description: Vec::new(),
            result: Vec::new(),
            children: Vec::new(),
            state: ItemState::Plain,
            commit: None,
        }
    }

    /// Spec 5.4: a bare title is coarse, a title with a description is sharp.
    pub fn is_sharpened(&self) -> bool {
        !self.description.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Marker {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Entry {
    Item(Item),
    Marker(Marker),
    Now,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub title: String,
    pub entries: Vec<Entry>,
}

/// How a command names a place on the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ref {
    Id(Id),
    Marker(String),
    Now,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Position {
    After(Ref),
    Before(Ref),
    End,
}

impl Line {
    pub fn now_index(&self) -> usize {
        self.entries
            .iter()
            .position(|e| matches!(e, Entry::Now))
            .expect("a Line always has exactly one Now; parsing guarantees it")
    }

    pub fn index_of(&self, r: &Ref) -> Option<usize> {
        self.entries.iter().position(|e| match (r, e) {
            (Ref::Now, Entry::Now) => true,
            (Ref::Id(want), Entry::Item(i)) => &i.id == want,
            (Ref::Marker(want), Entry::Marker(m)) => &m.label == want,
            _ => false,
        })
    }

    pub fn is_behind_now(&self, id: &Id) -> bool {
        match self.index_of(&Ref::Id(id.clone())) {
            Some(i) => i < self.now_index(),
            None => false,
        }
    }

    pub fn item(&self, id: &Id) -> Option<&Item> {
        self.entries.iter().find_map(|e| match e {
            Entry::Item(i) if &i.id == id => Some(i),
            _ => None,
        })
    }

    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.entries.iter().filter_map(|e| match e {
            Entry::Item(i) => Some(i),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Line {
        Line {
            title: "Test".into(),
            entries: vec![
                Entry::Item(Item::new(Id::new("aaa"), "past work")),
                Entry::Now,
                Entry::Marker(Marker {
                    label: "v0.1".into(),
                }),
                Entry::Item(Item::new(Id::new("bbb"), "future work")),
            ],
        }
    }

    #[test]
    fn now_index_finds_the_now_entry() {
        assert_eq!(line().now_index(), 1);
    }

    #[test]
    fn behind_now_is_position_not_a_flag() {
        let l = line();
        assert!(l.is_behind_now(&Id::new("aaa")));
        assert!(!l.is_behind_now(&Id::new("bbb")));
    }

    #[test]
    fn refs_resolve_to_indices() {
        let l = line();
        assert_eq!(l.index_of(&Ref::Id(Id::new("bbb"))), Some(3));
        assert_eq!(l.index_of(&Ref::Marker("v0.1".into())), Some(2));
        assert_eq!(l.index_of(&Ref::Now), Some(1));
        assert_eq!(l.index_of(&Ref::Id(Id::new("zzz"))), None);
    }

    #[test]
    fn item_lookup_returns_the_item() {
        assert_eq!(line().item(&Id::new("aaa")).unwrap().title, "past work");
    }
}

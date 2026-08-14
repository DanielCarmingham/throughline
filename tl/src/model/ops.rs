use super::{Entry, Id, ItemState, Line, Position, Ref};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OpError {
    #[error("no entry matches {0}")]
    UnknownRef(String),
    #[error("nothing ahead of Now to advance past")]
    NothingAhead,
    #[error("no item with id {0}")]
    UnknownItem(String),
}

fn describe(r: &Ref) -> String {
    match r {
        Ref::Id(id) => format!("^{}", id.0),
        Ref::Marker(l) => format!("marker {l:?}"),
        Ref::Now => "NOW".into(),
    }
}

impl Line {
    /// Resolve a Position to the index the new entry should occupy.
    fn target_index(&self, p: &Position) -> Result<usize, OpError> {
        Ok(match p {
            Position::End => self.entries.len(),
            Position::After(r) => {
                self.index_of(r).ok_or_else(|| OpError::UnknownRef(describe(r)))? + 1
            }
            Position::Before(r) => {
                self.index_of(r).ok_or_else(|| OpError::UnknownRef(describe(r)))?
            }
        })
    }

    pub fn insert(&mut self, entry: Entry, p: &Position) -> Result<(), OpError> {
        let at = self.target_index(p)?;
        self.entries.insert(at, entry);
        Ok(())
    }

    pub fn move_entry(&mut self, what: &Ref, p: &Position) -> Result<(), OpError> {
        let from = self
            .index_of(what)
            .ok_or_else(|| OpError::UnknownRef(describe(what)))?;
        // Resolve the destination BEFORE removing, then correct for the shift.
        let to = self.target_index(p)?;
        let entry = self.entries.remove(from);
        let to = if to > from { to - 1 } else { to };
        self.entries.insert(to, entry);
        Ok(())
    }

    /// Move Now forward. With no target, past the next item; with a target,
    /// past everything up to and including it. Returns the items passed.
    pub fn advance(&mut self, target: Option<&Ref>) -> Result<Vec<Id>, OpError> {
        let now = self.now_index();
        let stop = match target {
            Some(r) => self
                .index_of(r)
                .ok_or_else(|| OpError::UnknownRef(describe(r)))?,
            None => self.entries[now + 1..]
                .iter()
                .position(|e| matches!(e, Entry::Item(_)))
                .map(|offset| now + 1 + offset)
                .ok_or(OpError::NothingAhead)?,
        };
        if stop <= now {
            return Err(OpError::NothingAhead);
        }
        let passed = self.entries[now + 1..=stop]
            .iter()
            .filter_map(|e| match e {
                Entry::Item(i) => Some(i.id.clone()),
                _ => None,
            })
            .collect();
        let now_entry = self.entries.remove(now);
        self.entries.insert(stop, now_entry);
        Ok(passed)
    }

    /// Complete out of order: move the item to immediately behind Now.
    pub fn complete(&mut self, id: &Id) -> Result<(), OpError> {
        self.move_entry(&Ref::Id(id.clone()), &Position::Before(Ref::Now))
    }

    pub fn drop_item(&mut self, id: &Id, reason: String) -> Result<(), OpError> {
        let idx = self
            .index_of(&Ref::Id(id.clone()))
            .ok_or_else(|| OpError::UnknownItem(id.0.clone()))?;
        if let Entry::Item(item) = &mut self.entries[idx] {
            item.state = ItemState::Dropped(reason);
        }
        self.complete(id)
    }
}

#[cfg(test)]
mod tests {
    use crate::model::*;

    fn line() -> Line {
        Line {
            title: "Test".into(),
            entries: vec![
                Entry::Item(Item::new(Id::new("aaa"), "a")),
                Entry::Now,
                Entry::Item(Item::new(Id::new("bbb"), "b")),
                Entry::Marker(Marker { label: "v0.1".into() }),
                Entry::Item(Item::new(Id::new("ccc"), "c")),
            ],
        }
    }

    fn titles(l: &Line) -> Vec<String> {
        l.entries
            .iter()
            .map(|e| match e {
                Entry::Item(i) => i.title.clone(),
                Entry::Marker(m) => format!("<{}>", m.label),
                Entry::Now => "NOW".into(),
            })
            .collect()
    }

    #[test]
    fn insert_after_places_the_entry_immediately_after() {
        let mut l = line();
        l.insert(
            Entry::Item(Item::new(Id::new("ddd"), "d")),
            &Position::After(Ref::Id(Id::new("bbb"))),
        )
        .unwrap();
        assert_eq!(titles(&l), ["a", "NOW", "b", "d", "<v0.1>", "c"]);
    }

    #[test]
    fn insert_before_a_marker_places_work_ahead_of_the_landmark() {
        let mut l = line();
        l.insert(
            Entry::Item(Item::new(Id::new("ddd"), "d")),
            &Position::Before(Ref::Marker("v0.1".into())),
        )
        .unwrap();
        assert_eq!(titles(&l), ["a", "NOW", "b", "d", "<v0.1>", "c"]);
    }

    #[test]
    fn insert_at_end_appends() {
        let mut l = line();
        l.insert(Entry::Item(Item::new(Id::new("ddd"), "d")), &Position::End)
            .unwrap();
        assert_eq!(titles(&l), ["a", "NOW", "b", "<v0.1>", "c", "d"]);
    }

    #[test]
    fn insert_with_an_unknown_ref_errors() {
        let mut l = line();
        let err = l
            .insert(
                Entry::Item(Item::new(Id::new("ddd"), "d")),
                &Position::After(Ref::Id(Id::new("zzz"))),
            )
            .unwrap_err();
        assert!(matches!(err, OpError::UnknownRef(_)));
    }

    #[test]
    fn move_entry_reorders_without_duplicating() {
        let mut l = line();
        l.move_entry(&Ref::Id(Id::new("ccc")), &Position::After(Ref::Now))
            .unwrap();
        assert_eq!(titles(&l), ["a", "NOW", "c", "b", "<v0.1>"]);
    }

    #[test]
    fn move_entry_backwards_across_now_makes_it_history() {
        let mut l = line();
        l.move_entry(&Ref::Id(Id::new("ccc")), &Position::Before(Ref::Now))
            .unwrap();
        assert!(l.is_behind_now(&Id::new("ccc")));
    }

    #[test]
    fn advance_moves_now_past_the_next_item() {
        let mut l = line();
        let passed = l.advance(None).unwrap();
        assert_eq!(passed, vec![Id::new("bbb")]);
        assert_eq!(titles(&l), ["a", "b", "NOW", "<v0.1>", "c"]);
    }

    #[test]
    fn advance_to_a_target_carries_past_markers_in_between() {
        let mut l = line();
        let passed = l.advance(Some(&Ref::Id(Id::new("ccc")))).unwrap();
        assert_eq!(passed, vec![Id::new("bbb"), Id::new("ccc")]);
        assert_eq!(titles(&l), ["a", "b", "<v0.1>", "c", "NOW"]);
    }

    #[test]
    fn advance_past_the_end_errors() {
        let mut l = Line {
            title: "T".into(),
            entries: vec![Entry::Item(Item::new(Id::new("aaa"), "a")), Entry::Now],
        };
        assert!(matches!(l.advance(None).unwrap_err(), OpError::NothingAhead));
    }

    #[test]
    fn complete_moves_an_item_to_just_behind_now() {
        let mut l = line();
        l.complete(&Id::new("ccc")).unwrap();
        assert_eq!(titles(&l), ["a", "c", "NOW", "b", "<v0.1>"]);
        assert!(l.is_behind_now(&Id::new("ccc")));
    }

    #[test]
    fn drop_records_a_reason_and_moves_the_item_behind_now() {
        let mut l = line();
        l.drop_item(&Id::new("bbb"), "superseded".into()).unwrap();
        assert!(l.is_behind_now(&Id::new("bbb")));
        assert_eq!(
            l.item(&Id::new("bbb")).unwrap().state,
            ItemState::Dropped("superseded".into())
        );
    }
}

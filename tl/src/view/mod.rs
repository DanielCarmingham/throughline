use crate::config::Config;
use crate::model::{Entry, Item, Line, Ref};

/// A half-open range of entry indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn entries<'a>(&self, line: &'a Line) -> &'a [Entry] {
        &line.entries[self.start..self.end.min(line.entries.len())]
    }
}

/// Step outward from Now counting ITEMS, not entries: markers are landmarks and
/// should not consume window budget.
fn walk(line: &Line, from: usize, items: usize, forward: bool) -> usize {
    let mut idx = from;
    let mut left = items;
    loop {
        let next = if forward {
            if idx + 1 >= line.entries.len() {
                return line.entries.len().saturating_sub(1);
            }
            idx + 1
        } else {
            if idx == 0 {
                return 0;
            }
            idx - 1
        };
        idx = next;
        if matches!(line.entries[idx], Entry::Item(_)) {
            if left == 0 {
                return idx;
            }
            left -= 1;
        }
    }
}

pub fn window(line: &Line, cfg: &Config) -> Span {
    let now = line.now_index();
    let start = walk(line, now, cfg.window_back.saturating_sub(1), false);
    let end = walk(line, now, cfg.window_ahead.saturating_sub(1), true);
    Span {
        start,
        end: (end + 1).min(line.entries.len()),
    }
}

pub fn slice(line: &Line, from: &Ref, to: &Ref) -> Option<Span> {
    let a = line.index_of(from)?;
    let b = line.index_of(to)?;
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    Some(Span { start: lo, end: hi + 1 })
}

/// The next item ahead of Now — where work is happening.
pub fn at_now(line: &Line) -> Option<&Item> {
    line.entries[line.now_index() + 1..]
        .iter()
        .find_map(|e| match e {
            Entry::Item(i) => Some(i),
            _ => None,
        })
}

/// Item-counted distance from Now. Negative behind, positive ahead. Used for
/// the progressive-detail fade (spec 7.3).
pub fn distance_from_now(line: &Line, index: usize) -> isize {
    let now = line.now_index();
    let (lo, hi, sign) = if index < now {
        (index, now, -1)
    } else {
        (now, index, 1)
    };
    let count = line.entries[lo..hi]
        .iter()
        .filter(|e| matches!(e, Entry::Item(_)))
        .count() as isize;
    count * sign
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;
    use crate::model::Id;

    fn long_line() -> Line {
        let mut src = String::from("# T\n\n");
        for n in 0..6 {
            src.push_str(&format!("- [x] past{n}  ^p{n}\n"));
        }
        src.push_str("\n── NOW ──\n\n");
        for n in 0..12 {
            src.push_str(&format!("- [ ] fut{n}  ^f{n}\n"));
        }
        parse(&src).unwrap()
    }

    #[test]
    fn window_spans_config_back_and_ahead_around_now() {
        let l = long_line();
        let c = Config::default(); // back 3, ahead 7
        let w = window(&l, &c);
        assert_eq!(w.start, 3);
        assert_eq!(w.end, 14); // now at 6, +7 items, end exclusive
        assert_eq!(w.entries(&l).len(), 11);
    }

    #[test]
    fn window_clamps_at_the_start_of_the_line() {
        let l = parse("# T\n\n- [x] a  ^aaa\n\n── NOW ──\n\n- [ ] b  ^bbb\n").unwrap();
        let w = window(&l, &Config::default());
        assert_eq!(w.start, 0);
        assert_eq!(w.end, 3);
    }

    #[test]
    fn at_now_returns_the_next_item_ahead() {
        let l = long_line();
        assert_eq!(at_now(&l).unwrap().title, "fut0");
    }

    #[test]
    fn at_now_is_none_when_nothing_is_ahead() {
        let l = parse("# T\n\n- [x] a  ^aaa\n\n── NOW ──\n").unwrap();
        assert!(at_now(&l).is_none());
    }

    #[test]
    fn slice_between_two_refs_is_inclusive_of_both() {
        let l = long_line();
        let s = slice(&l, &Ref::Id(Id::new("p1")), &Ref::Id(Id::new("p3"))).unwrap();
        assert_eq!(s.entries(&l).len(), 3);
    }

    #[test]
    fn slice_accepts_now_as_an_endpoint() {
        let l = long_line();
        let s = slice(&l, &Ref::Id(Id::new("p4")), &Ref::Now).unwrap();
        assert_eq!(s.entries(&l).len(), 3);
    }

    #[test]
    fn slice_with_reversed_endpoints_still_returns_the_span() {
        let l = long_line();
        let s = slice(&l, &Ref::Id(Id::new("p3")), &Ref::Id(Id::new("p1"))).unwrap();
        assert_eq!(s.entries(&l).len(), 3);
    }

    #[test]
    fn slice_with_an_unknown_ref_is_none() {
        let l = long_line();
        assert!(slice(&l, &Ref::Id(Id::new("zzz")), &Ref::Now).is_none());
    }

    #[test]
    fn distance_is_negative_behind_now_and_positive_ahead() {
        let l = long_line();
        assert!(distance_from_now(&l, 0) < 0);
        assert_eq!(distance_from_now(&l, 6), 0);
        assert!(distance_from_now(&l, 10) > 0);
    }
}

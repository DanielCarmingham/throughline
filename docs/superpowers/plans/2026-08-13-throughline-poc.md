# Throughline POC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `tl`, a Rust CLI+TUI that manages a project as one ordered line through past, Now, and future, plus the method document it implements.

**Architecture:** A single Rust binary over one hand-editable Markdown file (`.throughline/line.md`). Pure layers — `model` (ordering), `format` (parse/serialize), `view` (window/slice), `check` (lints) — have no I/O and no terminal knowledge. Presentation layers — `glyphs`, `theme` — are pure lookups consumed by `cli` and `tui`, which are the only modules that touch a terminal.

**Tech Stack:** Rust 2021, `clap`, `ratatui` + `crossterm`, `serde`/`serde_json`, `toml`, `terminal-light`, `tui-textarea`, `anyhow`/`thiserror`; tests with `assert_cmd`, `predicates`, `insta`.

**Spec:** `docs/superpowers/specs/2026-08-13-throughline-poc-design.md`

## Global Constraints

Every task's requirements implicitly include this section. Values are copied verbatim from the spec.

- **Rust edition 2021**, minimum toolchain 1.80.
- **Canonical file syntax is unicode and never varies with glyph mode.** Write `── NOW ──` and `◆ label ◆`. Accept `-- NOW --` and `<> label <>` on read only. (spec 5.2)
- **Checkboxes are derived from position, never authored.** Above Now → `[x]`; below Now → `[ ]`; dropped → `[-]`. (spec 5.2)
- **Status is position.** There is no `Done` variant in any enum. Completion is `is_behind_now()`. (spec 3.1)
- **Window defaults:** `window_back` = 3, `window_ahead` = 7. **`far_body_lines`** = 3. All configurable in `.throughline/config.toml`. (spec 6.1)
- **Lint severity:** `bucket`, `unsharpened`, `false-certainty` are warnings and exit 0. All other lints are errors and exit non-zero. (spec 6.1)
- **Bucket vocabulary:** `backlog`, `someday`, `later`, `v2`, `post-launch`, `icebox`, `blocked`. Suppressible via `check.allow_markers`. (spec 6.1)
- **Only items behind Now may carry a result.** `false-certainty` counts description lines only. (spec 5.3)
- **Views may not construct a colour or a literal glyph.** Every style resolves from a `theme::Token`; every glyph from a `glyphs::Role`. (spec 4.3, 7.3)
- **Non-TTY output degrades automatically** to ascii glyphs with colour disabled. `NO_COLOR` respected. (spec 7.4)
- **Writes are atomic:** temp file in the same directory, then rename. (spec 4.1)
- **Parse errors carry line numbers.** (spec 4.1)
- **`@commit(rev)` prefers jj change IDs** when `.jj` is present; git SHAs otherwise. (spec 4.4)

## File Structure

A Cargo workspace at the repo root; the crate lives in `tl/` per spec 4.3.

| file | responsibility |
|---|---|
| `Cargo.toml` | workspace root |
| `tl/Cargo.toml` | crate manifest, `[[bin]] name = "tl"` |
| `tl/src/main.rs` | dispatch: no args → TUI, else CLI |
| `tl/src/model/mod.rs` | `Line`, `Entry`, `Item`, `Marker`, `Id`, `Ref`, `Position` |
| `tl/src/model/ops.rs` | `insert`, `move_entry`, `advance`, `complete`, `drop_item` |
| `tl/src/format/parse.rs` | `line.md` → `Line`, with line-numbered errors |
| `tl/src/format/write.rs` | `Line` → `line.md`, canonical + derived checkboxes |
| `tl/src/format/io.rs` | atomic read/write of the file on disk |
| `tl/src/view/mod.rs` | `window()`, `slice()`, `now()` — pure, no I/O |
| `tl/src/check/mod.rs` | lint definitions, severities, `Finding` |
| `tl/src/glyphs/mod.rs` | `Role`, three glyph sets, mode resolution |
| `tl/src/theme/mod.rs` | `Token`, dark/light palettes, colour-depth degradation |
| `tl/src/config.rs` | `.throughline/config.toml` + env + flag resolution |
| `tl/src/cli/mod.rs` | clap command tree |
| `tl/src/cli/render.rs` | text and JSON renderers for read commands |
| `tl/src/tui/ribbon.rs` | the horizontal whole-project ribbon |
| `tl/src/tui/list.rs` | the vertical window list |
| `tl/src/tui/app.rs` | app state, keymap, event loop |
| `tl/src/diagrams.rs` | render fixture lines as method-doc diagrams |
| `tl/tests/` | integration tests (`assert_cmd`) |
| `docs/method.md` | the method, with 15 diagrams |
| `.throughline/line.md` | the dogfooded line |

---

### Task 1: Workspace scaffold and the core model

**Files:**
- Create: `Cargo.toml`, `tl/Cargo.toml`, `tl/src/main.rs`, `tl/src/lib.rs`, `tl/src/model/mod.rs`
- Test: inline `#[cfg(test)]` in `tl/src/model/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `Id(String)`, `Item`, `Child`, `Marker`, `ItemState`, `Entry`, `Line`, `Ref`, `Position`; `Line::now_index() -> usize`, `Line::index_of(&Ref) -> Option<usize>`, `Line::is_behind_now(&Id) -> bool`, `Line::item(&Id) -> Option<&Item>`.

- [ ] **Step 1: Write the failing test**

Create `tl/src/model/mod.rs` with only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Line {
        Line {
            title: "Test".into(),
            entries: vec![
                Entry::Item(Item::new(Id::new("aaa"), "past work")),
                Entry::Now,
                Entry::Marker(Marker { label: "v0.1".into() }),
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl model`
Expected: FAIL — `cannot find type Line in this scope` and similar.

- [ ] **Step 3: Write minimal implementation**

`Cargo.toml` (workspace root):

```toml
[workspace]
members = ["tl"]
resolver = "2"
```

`tl/Cargo.toml`:

```toml
[package]
name = "tl"
version = "0.1.0"
edition = "2021"
rust-version = "1.80"

[[bin]]
name = "tl"
path = "src/main.rs"

[dependencies]
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
```

`tl/src/lib.rs`:

```rust
pub mod model;
```

`tl/src/main.rs`:

```rust
fn main() {
    println!("tl");
}
```

`tl/src/model/mod.rs` — prepend above the existing test module:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl model`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml tl/
git commit -m "feat(model): line, entries, and position queries"
```

---

### Task 2: Ordering operations

**Files:**
- Create: `tl/src/model/ops.rs`
- Modify: `tl/src/model/mod.rs` (add `pub mod ops;`)
- Test: inline `#[cfg(test)]` in `tl/src/model/ops.rs`

**Interfaces:**
- Consumes: `Line`, `Entry`, `Item`, `Id`, `Ref`, `Position`, `ItemState` from Task 1.
- Produces: `OpError`; `Line::insert(Entry, &Position) -> Result<(), OpError>`, `Line::move_entry(&Ref, &Position) -> Result<(), OpError>`, `Line::advance(Option<&Ref>) -> Result<Vec<Id>, OpError>`, `Line::complete(&Id) -> Result<(), OpError>`, `Line::drop_item(&Id, String) -> Result<(), OpError>`.

- [ ] **Step 1: Write the failing test**

Create `tl/src/model/ops.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl ops`
Expected: FAIL — `no method named insert found for struct Line`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod ops;` to `tl/src/model/mod.rs`, then prepend to `tl/src/model/ops.rs`:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl ops`
Expected: PASS, 11 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/src/model/
git commit -m "feat(model): ordering operations — insert, move, advance, complete, drop"
```

---

### Task 3: Parse `line.md`

**Files:**
- Create: `tl/src/format/mod.rs`, `tl/src/format/parse.rs`
- Modify: `tl/src/lib.rs` (add `pub mod format;`)
- Test: inline `#[cfg(test)]` in `tl/src/format/parse.rs`

**Interfaces:**
- Consumes: all of `model` from Tasks 1–2.
- Produces: `ParseError { line: usize, message: String }`, `parse(&str) -> Result<Line, ParseError>`.

- [ ] **Step 1: Write the failing test**

Create `tl/src/format/parse.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    const SAMPLE: &str = "\
# Throughline — Test

## Line

- [x] Sketch the method  ^k3f
      → Ten properties.
        The window idea is the original one.
- [x] Pick the name  ^m2a  @commit(88ca65b)

── NOW ──

- [ ] Write docs/method.md  ^q1d
      The full method: line, Now, window.
- [ ] Build account recovery  ^x2d
      - [ ] Generate recovery token
      - [x] Send recovery email

◆ v0.1 — tl renders the line ◆

- [ ] Ship it  ^t9a  @blocked(waiting on keys)
";

    #[test]
    fn reads_the_document_title() {
        assert_eq!(parse(SAMPLE).unwrap().title, "Throughline — Test");
    }

    #[test]
    fn entries_appear_in_file_order() {
        let l = parse(SAMPLE).unwrap();
        let shape: Vec<&str> = l
            .entries
            .iter()
            .map(|e| match e {
                Entry::Item(_) => "item",
                Entry::Marker(_) => "marker",
                Entry::Now => "now",
            })
            .collect();
        assert_eq!(
            shape,
            ["item", "item", "now", "item", "item", "marker", "item"]
        );
    }

    #[test]
    fn results_are_captured_and_continue_across_indented_lines() {
        let l = parse(SAMPLE).unwrap();
        assert_eq!(
            l.item(&Id::new("k3f")).unwrap().result,
            vec![
                "Ten properties.".to_string(),
                "The window idea is the original one.".to_string()
            ]
        );
    }

    #[test]
    fn descriptions_are_separate_from_results() {
        let l = parse(SAMPLE).unwrap();
        let item = l.item(&Id::new("q1d")).unwrap();
        assert_eq!(item.description, vec!["The full method: line, Now, window."]);
        assert!(item.result.is_empty());
    }

    #[test]
    fn children_are_parsed_and_have_no_ids() {
        let l = parse(SAMPLE).unwrap();
        let item = l.item(&Id::new("x2d")).unwrap();
        assert_eq!(item.children.len(), 2);
        assert_eq!(item.children[0].title, "Generate recovery token");
        assert!(!item.children[0].done);
        assert!(item.children[1].done);
    }

    #[test]
    fn inline_metadata_is_parsed() {
        let l = parse(SAMPLE).unwrap();
        assert_eq!(
            l.item(&Id::new("m2a")).unwrap().commit,
            Some("88ca65b".to_string())
        );
        assert_eq!(
            l.item(&Id::new("t9a")).unwrap().state,
            ItemState::Blocked("waiting on keys".into())
        );
    }

    #[test]
    fn markers_keep_their_label() {
        let l = parse(SAMPLE).unwrap();
        match &l.entries[5] {
            Entry::Marker(m) => assert_eq!(m.label, "v0.1 — tl renders the line"),
            other => panic!("expected marker, got {other:?}"),
        }
    }

    #[test]
    fn ascii_forms_are_accepted_on_read() {
        let src = "# T\n\n- [x] a  ^aaa\n\n-- NOW --\n\n<> v1 <>\n\n- [ ] b  ^bbb\n";
        let l = parse(src).unwrap();
        assert_eq!(l.now_index(), 1);
        match &l.entries[2] {
            Entry::Marker(m) => assert_eq!(m.label, "v1"),
            other => panic!("expected marker, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_now_is_an_error() {
        let err = parse("# T\n\n- [ ] a  ^aaa\n").unwrap_err();
        assert!(err.message.contains("NOW"));
    }

    #[test]
    fn a_duplicate_now_is_an_error_carrying_the_line_number() {
        let src = "# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n\n── NOW ──\n";
        let err = parse(src).unwrap_err();
        assert_eq!(err.line, 7);
    }

    #[test]
    fn an_item_without_an_id_is_an_error_carrying_the_line_number() {
        let src = "# T\n\n── NOW ──\n\n- [ ] no id here\n";
        let err = parse(src).unwrap_err();
        assert_eq!(err.line, 5);
        assert!(err.message.contains("id"));
    }

    #[test]
    fn a_duplicate_id_is_an_error() {
        let src = "# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n- [ ] b  ^aaa\n";
        let err = parse(src).unwrap_err();
        assert_eq!(err.line, 6);
        assert!(err.message.contains("duplicate"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl parse`
Expected: FAIL — `cannot find function parse in this scope`.

- [ ] **Step 3: Write minimal implementation**

`tl/src/format/mod.rs`:

```rust
pub mod parse;
pub use parse::{parse, ParseError};
```

Add `pub mod format;` to `tl/src/lib.rs`. Prepend to `tl/src/format/parse.rs`:

```rust
use crate::model::*;
use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError { line, message: message.into() }
}

/// `── NOW ──` (canonical) or `-- NOW --` (accepted).
fn is_now(t: &str) -> bool {
    let s = t.trim_matches(|c| c == '─' || c == '-' || c == ' ');
    s == "NOW" && t.len() > 3
}

/// `◆ label ◆` (canonical) or `<> label <>` (accepted).
fn marker_label(t: &str) -> Option<String> {
    if let Some(inner) = t.strip_prefix('◆').and_then(|s| s.strip_suffix('◆')) {
        return Some(inner.trim().to_string());
    }
    if let Some(inner) = t.strip_prefix("<>").and_then(|s| s.strip_suffix("<>")) {
        return Some(inner.trim().to_string());
    }
    None
}

/// Pull `^id`, `@commit(..)`, `@blocked(..)`, `@dropped(..)`, `@active` off a
/// title line, returning the bare title and the metadata found.
fn split_meta(text: &str) -> (String, Option<Id>, Option<String>, ItemState) {
    let mut title = Vec::new();
    let mut id = None;
    let mut commit = None;
    let mut state = ItemState::Plain;

    for tok in text.split_whitespace() {
        if let Some(rest) = tok.strip_prefix('^') {
            id = Some(Id::new(rest));
        } else if let Some(rest) = tok.strip_prefix("@commit(").and_then(|s| s.strip_suffix(')')) {
            commit = Some(rest.to_string());
        } else if tok == "@active" {
            state = ItemState::Active;
        } else if tok.starts_with("@blocked(") || tok.starts_with("@dropped(") {
            title.push(tok); // reassembled below
        } else {
            title.push(tok);
        }
    }

    // `@blocked(...)`/`@dropped(...)` may contain spaces, so recover them from
    // the raw text rather than from whitespace tokens.
    let mut bare = title.join(" ");
    for (tag, ctor) in [
        ("@blocked(", ItemState::Blocked as fn(String) -> ItemState),
        ("@dropped(", ItemState::Dropped as fn(String) -> ItemState),
    ] {
        if let Some(start) = bare.find(tag) {
            if let Some(end) = bare[start..].find(')') {
                let reason = bare[start + tag.len()..start + end].to_string();
                state = ctor(reason);
                bare.replace_range(start..start + end + 1, "");
            }
        }
    }

    (bare.trim().to_string(), id, commit, state)
}

pub fn parse(src: &str) -> Result<Line, ParseError> {
    let mut title = String::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut now_line: Option<usize> = None;
    // Tracks whether indented lines under the current item are result lines.
    let mut in_result = false;

    for (i, raw) in src.lines().enumerate() {
        let lineno = i + 1;
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("# ") {
            if title.is_empty() {
                title = h.trim().to_string();
            }
            continue;
        }
        if trimmed.starts_with("## ") || trimmed.starts_with("<!--") {
            continue;
        }
        if is_now(trimmed) {
            if let Some(first) = now_line {
                return Err(err(
                    lineno,
                    format!("a second NOW marker; the first is on line {first}"),
                ));
            }
            now_line = Some(lineno);
            entries.push(Entry::Now);
            in_result = false;
            continue;
        }
        if let Some(label) = marker_label(trimmed) {
            entries.push(Entry::Marker(Marker { label }));
            in_result = false;
            continue;
        }

        let indented = raw.starts_with("  ");

        // An indented checkbox is a child of the current item.
        if indented && (trimmed.starts_with("- [") ) {
            let done = trimmed.starts_with("- [x]");
            let text = trimmed[5..].trim().to_string();
            match entries.last_mut() {
                Some(Entry::Item(item)) => item.children.push(Child { title: text, done }),
                _ => return Err(err(lineno, "a child with no parent item above it")),
            }
            in_result = false;
            continue;
        }

        // A top-level checkbox is an item.
        if !indented && trimmed.starts_with("- [") {
            if trimmed.len() < 6 {
                return Err(err(lineno, "malformed item"));
            }
            let (bare, id, commit, state) = split_meta(trimmed[5..].trim());
            let id = id.ok_or_else(|| err(lineno, "item has no ^id"))?;
            if !seen_ids.insert(id.0.clone()) {
                return Err(err(lineno, format!("duplicate id ^{}", id.0)));
            }
            let mut item = Item::new(id, bare);
            item.commit = commit;
            item.state = state;
            entries.push(Entry::Item(item));
            in_result = false;
            continue;
        }

        // Indented prose belongs to the current item.
        if indented {
            let is_result_start = trimmed.starts_with('→') || trimmed.starts_with("->");
            let text = if is_result_start {
                in_result = true;
                trimmed
                    .trim_start_matches('→')
                    .trim_start_matches("->")
                    .trim()
                    .to_string()
            } else {
                trimmed.to_string()
            };
            match entries.last_mut() {
                Some(Entry::Item(item)) => {
                    if in_result {
                        item.result.push(text);
                    } else {
                        item.description.push(text);
                    }
                }
                _ => return Err(err(lineno, "indented text with no item above it")),
            }
            continue;
        }

        return Err(err(lineno, format!("unrecognised line: {trimmed:?}")));
    }

    if now_line.is_none() {
        return Err(err(
            src.lines().count().max(1),
            "no NOW marker; every line must have exactly one",
        ));
    }

    Ok(Line { title, entries })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl parse`
Expected: PASS, 12 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/src/format/ tl/src/lib.rs
git commit -m "feat(format): parse line.md with line-numbered errors"
```

---

### Task 4: Serialize and normalize

**Files:**
- Create: `tl/src/format/write.rs`
- Modify: `tl/src/format/mod.rs` (add `pub mod write; pub use write::render;`)
- Test: inline `#[cfg(test)]` in `tl/src/format/write.rs`

**Interfaces:**
- Consumes: `parse` from Task 3, all of `model`.
- Produces: `render(&Line) -> String`.

Checkboxes are derived here and nowhere else. Rendering is the only place that
knows `[x]` exists.

- [ ] **Step 1: Write the failing test**

Create `tl/src/format/write.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::parse;
    use crate::model::*;

    const SAMPLE: &str = "\
# Throughline — Test

## Line

- [x] Sketch the method  ^k3f
      → Ten properties.
        The window idea is the original one.

── NOW ──

- [ ] Write docs/method.md  ^q1d
      The full method: line, Now, window.
- [ ] Build account recovery  ^x2d
      - [ ] Generate recovery token
      - [x] Send recovery email

◆ v0.1 ◆

- [ ] Ship it  ^t9a  @blocked(waiting on keys)
";

    #[test]
    fn round_trips_through_parse() {
        let once = parse(SAMPLE).unwrap();
        let twice = parse(&render(&once)).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn rendering_is_idempotent() {
        let l = parse(SAMPLE).unwrap();
        let first = render(&l);
        let second = render(&parse(&first).unwrap());
        assert_eq!(first, second);
    }

    #[test]
    fn checkboxes_are_derived_from_position_not_from_the_source() {
        // Authored with the WRONG boxes: a past item unchecked, a future one checked.
        let wrong = "# T\n\n- [ ] past  ^aaa\n\n── NOW ──\n\n- [x] future  ^bbb\n";
        let out = render(&parse(wrong).unwrap());
        assert!(out.contains("- [x] past  ^aaa"));
        assert!(out.contains("- [ ] future  ^bbb"));
    }

    #[test]
    fn moving_an_item_across_now_changes_its_checkbox() {
        let mut l = parse("# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n").unwrap();
        l.complete(&Id::new("aaa")).unwrap();
        assert!(render(&l).contains("- [x] a  ^aaa"));
    }

    #[test]
    fn dropped_items_render_with_a_dash_box_and_keep_their_reason() {
        let mut l = parse("# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n").unwrap();
        l.drop_item(&Id::new("aaa"), "superseded".into()).unwrap();
        let out = render(&l);
        assert!(out.contains("- [-] a  ^aaa  @dropped(superseded)"));
    }

    #[test]
    fn ascii_input_is_normalized_to_canonical_unicode() {
        let ascii = "# T\n\n- [x] a  ^aaa\n\n-- NOW --\n\n<> v1 <>\n\n- [ ] b  ^bbb\n";
        let out = render(&parse(ascii).unwrap());
        assert!(out.contains("── NOW ──"));
        assert!(out.contains("◆ v1 ◆"));
        assert!(!out.contains("-- NOW --"));
        assert!(!out.contains("<> v1 <>"));
    }

    #[test]
    fn results_render_with_an_arrow_and_hanging_indent() {
        let out = render(&parse(SAMPLE).unwrap());
        assert!(out.contains("      → Ten properties."));
        assert!(out.contains("        The window idea is the original one."));
    }

    #[test]
    fn children_render_indented_and_keep_their_own_boxes() {
        let out = render(&parse(SAMPLE).unwrap());
        assert!(out.contains("      - [ ] Generate recovery token"));
        assert!(out.contains("      - [x] Send recovery email"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl write`
Expected: FAIL — `cannot find function render in this scope`.

- [ ] **Step 3: Write minimal implementation**

Update `tl/src/format/mod.rs`:

```rust
pub mod parse;
pub mod write;
pub use parse::{parse, ParseError};
pub use write::render;
```

Prepend to `tl/src/format/write.rs`:

```rust
use crate::model::*;

const BODY_INDENT: &str = "      ";
const HANG_INDENT: &str = "        ";

pub fn render(line: &Line) -> String {
    let now = line.now_index();
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n## Line\n\n", line.title));

    for (i, entry) in line.entries.iter().enumerate() {
        match entry {
            Entry::Now => out.push_str("\n── NOW ──\n\n"),
            Entry::Marker(m) => out.push_str(&format!("\n◆ {} ◆\n\n", m.label)),
            Entry::Item(item) => out.push_str(&render_item(item, i < now)),
        }
    }

    // Collapse any run of blank lines introduced around Now and markers.
    let mut squeezed = String::new();
    let mut blanks = 0;
    for l in out.lines() {
        if l.trim().is_empty() {
            blanks += 1;
            if blanks > 1 {
                continue;
            }
        } else {
            blanks = 0;
        }
        squeezed.push_str(l);
        squeezed.push('\n');
    }
    squeezed
}

fn render_item(item: &Item, behind_now: bool) -> String {
    let box_ = match (&item.state, behind_now) {
        (ItemState::Dropped(_), _) => "[-]",
        (_, true) => "[x]",
        (_, false) => "[ ]",
    };

    let mut head = format!("- {} {}  ^{}", box_, item.title, item.id.0);
    if let Some(c) = &item.commit {
        head.push_str(&format!("  @commit({c})"));
    }
    match &item.state {
        ItemState::Plain => {}
        ItemState::Active => head.push_str("  @active"),
        ItemState::Blocked(r) => head.push_str(&format!("  @blocked({r})")),
        ItemState::Dropped(r) => head.push_str(&format!("  @dropped({r})")),
    }
    head.push('\n');

    for d in &item.description {
        head.push_str(&format!("{BODY_INDENT}{d}\n"));
    }
    for c in &item.children {
        let b = if c.done { "[x]" } else { "[ ]" };
        head.push_str(&format!("{BODY_INDENT}- {} {}\n", b, c.title));
    }
    for (n, r) in item.result.iter().enumerate() {
        if n == 0 {
            head.push_str(&format!("{BODY_INDENT}→ {r}\n"));
        } else {
            head.push_str(&format!("{HANG_INDENT}{r}\n"));
        }
    }
    head
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl write`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/src/format/
git commit -m "feat(format): render line.md with derived checkboxes and canonical syntax"
```

---

### Task 5: Config and atomic file I/O

**Files:**
- Create: `tl/src/config.rs`, `tl/src/format/io.rs`
- Modify: `tl/Cargo.toml` (add `toml`, `dirs`, `tempfile`), `tl/src/lib.rs`, `tl/src/format/mod.rs`
- Test: inline `#[cfg(test)]` in both new files

**Interfaces:**
- Consumes: `parse`, `render` from Tasks 3–4.
- Produces: `Config { window_back: usize, window_ahead: usize, far_body_lines: usize, allow_markers: Vec<String>, glyphs: Option<String>, theme: Option<String> }`, `Config::load(&Path) -> Config`; `io::find_line_file(&Path) -> Option<PathBuf>`, `io::read(&Path) -> anyhow::Result<Line>`, `io::write_atomic(&Path, &Line) -> anyhow::Result<()>`.

- [ ] **Step 1: Write the failing test**

Create `tl/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_spec() {
        let c = Config::default();
        assert_eq!(c.window_back, 3);
        assert_eq!(c.window_ahead, 7);
        assert_eq!(c.far_body_lines, 3);
        assert!(c.allow_markers.is_empty());
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
        assert_eq!(c.allow_markers, vec!["v2".to_string()]);
    }
}
```

Create `tl/src/format/io.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::parse;

    #[test]
    fn finds_the_line_file_by_walking_up() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".throughline")).unwrap();
        std::fs::write(root.path().join(".throughline/line.md"), "# T\n\n── NOW ──\n").unwrap();
        let deep = root.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();

        let found = find_line_file(&deep).unwrap();
        assert_eq!(found, root.path().join(".throughline/line.md"));
    }

    #[test]
    fn returns_none_when_there_is_no_line_file() {
        let root = tempfile::tempdir().unwrap();
        assert!(find_line_file(root.path()).is_none());
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("line.md");
        let l = parse("# T\n\n- [x] a  ^aaa\n\n── NOW ──\n\n- [ ] b  ^bbb\n").unwrap();

        write_atomic(&path, &l).unwrap();
        assert_eq!(read(&path).unwrap(), l);
    }

    #[test]
    fn write_leaves_no_temp_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("line.md");
        let l = parse("# T\n\n── NOW ──\n").unwrap();

        write_atomic(&path, &l).unwrap();

        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["line.md".to_string()]);
    }

    #[test]
    fn a_parse_error_surfaces_the_path_and_line_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("line.md");
        std::fs::write(&path, "# T\n\n── NOW ──\n\n- [ ] no id\n").unwrap();

        let msg = read(&path).unwrap_err().to_string();
        assert!(msg.contains("line.md:5"), "got: {msg}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl config io`
Expected: FAIL — `cannot find type Config`, `cannot find function find_line_file`.

- [ ] **Step 3: Write minimal implementation**

Add to `tl/Cargo.toml` under `[dependencies]`:

```toml
toml = "0.8"
dirs = "5"
tempfile = "3"
```

Add to `tl/src/lib.rs`:

```rust
pub mod config;
```

Add to `tl/src/format/mod.rs`:

```rust
pub mod io;
```

Prepend to `tl/src/config.rs`:

```rust
use serde::Deserialize;
use std::path::Path;

fn d_back() -> usize { 3 }
fn d_ahead() -> usize { 7 }
fn d_far() -> usize { 3 }

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(default = "d_back")]
    pub window_back: usize,
    #[serde(default = "d_ahead")]
    pub window_ahead: usize,
    #[serde(default = "d_far")]
    pub far_body_lines: usize,
    pub glyphs: Option<String>,
    pub theme: Option<String>,
    #[serde(rename = "check", deserialize_with = "check_allow_markers", default)]
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
            allow_markers: Vec::new(),
        }
    }
}

#[derive(Deserialize, Default)]
struct CheckSection {
    #[serde(default)]
    allow_markers: Vec<String>,
}

fn check_allow_markers<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = CheckSection::deserialize(d)?;
    Ok(s.allow_markers)
}

impl Config {
    /// Project config, then user config, then defaults. First hit wins.
    pub fn load(start: &Path) -> Config {
        let project = start.join(".throughline/config.toml");
        if let Ok(text) = std::fs::read_to_string(&project) {
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
```

Prepend to `tl/src/format/io.rs`:

```rust
use crate::format::{parse, render};
use crate::model::Line;
use anyhow::{anyhow, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Walk up from `start` looking for `.throughline/line.md`.
pub fn find_line_file(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(".throughline/line.md");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

pub fn read(path: &Path) -> Result<Line> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    parse(&text).map_err(|e| {
        anyhow!(
            "{}:{}: {}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            e.line,
            e.message
        )
    })
}

/// Write to a temp file in the SAME directory, then rename. A rename within a
/// directory is atomic, so a crash mid-write can never truncate the line.
pub fn write_atomic(path: &Path, line: &Line) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).ok();
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(render(line).as_bytes())?;
    tmp.flush()?;
    tmp.persist(path)
        .map_err(|e| anyhow!("persisting {}: {}", path.display(), e))?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl config io`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/src/config.rs tl/src/format/ tl/Cargo.toml tl/src/lib.rs
git commit -m "feat: config resolution and atomic line.md I/O"
```

---

### Task 6: Views — now, window, slice

**Files:**
- Create: `tl/src/view/mod.rs`
- Modify: `tl/src/lib.rs` (add `pub mod view;`)
- Test: inline `#[cfg(test)]` in `tl/src/view/mod.rs`

**Interfaces:**
- Consumes: `Line`, `Entry`, `Id`, `Ref` from Tasks 1–2; `Config` from Task 5.
- Produces: `Span { pub start: usize, pub end: usize }` (half-open), `view::window(&Line, &Config) -> Span`, `view::slice(&Line, &Ref, &Ref) -> Option<Span>`, `view::at_now(&Line) -> Option<&Item>`, `Span::entries<'a>(&self, &'a Line) -> &'a [Entry]`, `view::distance_from_now(&Line, usize) -> isize`.

The Window is derived, never stored (spec 3.3). It is measured from Now, not
from a scroll position, so lints computed against it are stable.

- [ ] **Step 1: Write the failing test**

Create `tl/src/view/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl view`
Expected: FAIL — `cannot find function window in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod view;` to `tl/src/lib.rs`. Prepend to `tl/src/view/mod.rs`:

```rust
use crate::config::Config;
use crate::model::{Entry, Item, Line, Ref};

#[cfg(test)]
use crate::model::Id;

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
                return line.entries.len();
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
    Span { start, end: (end + 1).min(line.entries.len()) }
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
    let (lo, hi, sign) = if index < now { (index, now, -1) } else { (now, index, 1) };
    let count = line.entries[lo..hi]
        .iter()
        .filter(|e| matches!(e, Entry::Item(_)))
        .count() as isize;
    count * sign
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl view`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/src/view/ tl/src/lib.rs
git commit -m "feat(view): derived window, slice, and distance-from-now"
```

---

### Task 7: Glyph sets

**Files:**
- Create: `tl/src/glyphs/mod.rs`
- Modify: `tl/src/lib.rs` (add `pub mod glyphs;`)
- Test: inline `#[cfg(test)]` in `tl/src/glyphs/mod.rs`

**Interfaces:**
- Consumes: `Config` from Task 5.
- Produces: `Role` (18 variants), `Mode { NerdFont, Unicode, Ascii }`, `Glyphs`, `Glyphs::for_mode(Mode) -> Glyphs`, `Glyphs::get(Role) -> &'static str`, `Mode::resolve(flag: Option<&str>, cfg: &Config, is_tty: bool) -> Mode`.

Codepoints are from Nerd Fonts `glyphnames.json` v3.5.0 and are listed in spec
7.1. Do not substitute other glyphs: `pass_filled` and `circle_large` are
optically the same size, which is why items do not jitter as they cross Now.

- [ ] **Step 1: Write the failing test**

Create `tl/src/glyphs/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    const ALL: [Role; 18] = [
        Role::Done, Role::Open, Role::Active, Role::Dropped, Role::Blocked,
        Role::Marker, Role::Now, Role::Arrow, Role::Children, Role::Sharpened,
        Role::Coarse, Role::Cycle, Role::History, Role::ZoomOut, Role::Search,
        Role::WindowLeft, Role::WindowRight, Role::Rule,
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
            assert!(g.get(role).is_ascii(), "{role:?} is not ascii: {:?}", g.get(role));
        }
    }

    #[test]
    fn nerdfont_codepoints_match_the_spec() {
        let g = Glyphs::for_mode(Mode::NerdFont);
        assert_eq!(g.get(Role::Done), "\u{ebb3}");   // cod-pass_filled
        assert_eq!(g.get(Role::Open), "\u{ebb5}");   // cod-circle_large
        assert_eq!(g.get(Role::Now), "\u{eb1a}");    // cod-location
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl glyphs`
Expected: FAIL — `cannot find type Role in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod glyphs;` to `tl/src/lib.rs`. Prepend to `tl/src/glyphs/mod.rs`:

```rust
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Done, Open, Active, Dropped, Blocked,
    Marker, Now, Arrow, Children, Sharpened,
    Coarse, Cycle, History, ZoomOut, Search,
    WindowLeft, WindowRight, Rule,
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl glyphs`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/src/glyphs/ tl/src/lib.rs
git commit -m "feat(glyphs): nerdfont, unicode, and ascii sets with mode resolution"
```

---

### Task 8: Theme tokens and palettes

**Files:**
- Create: `tl/src/theme/mod.rs`
- Modify: `tl/Cargo.toml` (add `ratatui`, `terminal-light`), `tl/src/lib.rs`
- Test: inline `#[cfg(test)]` in `tl/src/theme/mod.rs`

**Interfaces:**
- Consumes: `Config` from Task 5.
- Produces: `Token` (13 variants), `Variant { Dark, Light }`, `Depth { True, Ansi256, Ansi16, None }`, `Theme`, `Theme::new(Variant, Depth) -> Theme`, `Theme::style(Token) -> ratatui::style::Style`, `Theme::fade(isize) -> Token`, `Variant::resolve(Option<&str>, &Config) -> Variant`, `Depth::detect(is_tty: bool) -> Depth`.

The light theme is not an inversion: bright cyan on white is unreadable, so it
takes a deeper accent (spec 7.2). `near`/`mid`/`far` are theme-authored because
dark fades toward black and light fades toward white (spec 7.3).

- [ ] **Step 1: Write the failing test**

Create `tl/src/theme/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use ratatui::style::Color;

    const ALL: [Token; 13] = [
        Token::Past, Token::Now, Token::Near, Token::Mid, Token::Far,
        Token::Marker, Token::Blocked, Token::Dropped, Token::Cursor,
        Token::Window, Token::Muted, Token::Bg, Token::Fg,
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
        let dark = Theme::new(Variant::Dark, Depth::True).style(Token::Now).fg.unwrap();
        let light = Theme::new(Variant::Light, Depth::True).style(Token::Now).fg.unwrap();
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl theme`
Expected: FAIL — `cannot find type Token in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add to `tl/Cargo.toml` under `[dependencies]`:

```toml
ratatui = "0.29"
crossterm = "0.28"
terminal-light = "1.4"
```

Add `pub mod theme;` to `tl/src/lib.rs`. Prepend to `tl/src/theme/mod.rs`:

```rust
use crate::config::Config;
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Past, Now, Near, Mid, Far,
    Marker, Blocked, Dropped, Cursor, Window, Muted, Bg, Fg,
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl theme`
Expected: PASS, 9 tests. (The colour lint passes trivially now and becomes
load-bearing once `cli/render.rs` and `tui/` exist in Tasks 10–13.)

- [ ] **Step 5: Commit**

```bash
git add tl/src/theme/ tl/Cargo.toml tl/src/lib.rs
git commit -m "feat(theme): semantic tokens with dark and light palettes"
```

---

### Task 9: Method lints (`tl check`)

**Files:**
- Create: `tl/src/check/mod.rs`
- Modify: `tl/src/lib.rs` (add `pub mod check;`)
- Test: inline `#[cfg(test)]` in `tl/src/check/mod.rs`

**Interfaces:**
- Consumes: `Line`, `Item`, `Entry`, `ItemState` from Tasks 1–2; `Config` from Task 5; `view::window`, `view::distance_from_now` from Task 6.
- Produces: `Severity { Warning, Error }`, `Finding { pub lint: &'static str, pub severity: Severity, pub subject: String, pub message: String }`, `check(&Line, &Config) -> Vec<Finding>`, `has_errors(&[Finding]) -> bool`.

`duplicate-id` and `no-now` are structurally impossible to reach through
`parse`, which rejects both. They are implemented here anyway because a `Line`
can also be built in memory, and because `check` is the documented contract.

- [ ] **Step 1: Write the failing test**

Create `tl/src/check/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;

    fn lints(src: &str) -> Vec<String> {
        check(&parse(src).unwrap(), &Config::default())
            .into_iter()
            .map(|f| f.lint.to_string())
            .collect()
    }

    #[test]
    fn a_bucket_marker_warns() {
        let out = check(
            &parse("# T\n\n── NOW ──\n\n◆ backlog ◆\n\n- [ ] a  ^aaa\n      body\n").unwrap(),
            &Config::default(),
        );
        let f = out.iter().find(|f| f.lint == "bucket").expect("no bucket lint");
        assert_eq!(f.severity, Severity::Warning);
    }

    #[test]
    fn the_allowlist_suppresses_the_bucket_lint() {
        let mut cfg = Config::default();
        cfg.allow_markers = vec!["v2".into()];
        let l = parse("# T\n\n── NOW ──\n\n◆ v2 ◆\n\n- [ ] a  ^aaa\n      body\n").unwrap();
        assert!(!check(&l, &cfg).iter().any(|f| f.lint == "bucket"));
    }

    #[test]
    fn a_bare_item_inside_the_window_is_unsharpened() {
        assert!(lints("# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n").contains(&"unsharpened".into()));
    }

    #[test]
    fn a_sharpened_item_inside_the_window_is_clean() {
        let out = lints("# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n      why this matters\n");
        assert!(!out.contains(&"unsharpened".into()));
    }

    #[test]
    fn a_long_body_far_from_now_is_false_certainty() {
        let mut src = String::from("# T\n\n── NOW ──\n\n");
        for n in 0..9 {
            src.push_str(&format!("- [ ] near{n}  ^n{n}\n      body\n"));
        }
        src.push_str("- [ ] distant  ^ddd\n      one\n      two\n      three\n      four\n");
        assert!(lints(&src).contains(&"false-certainty".into()));
    }

    #[test]
    fn results_do_not_count_toward_false_certainty() {
        // A long RESULT far behind Now is history and always allowed.
        let src = "# T\n\n- [x] a  ^aaa\n      → one\n        two\n        three\n        four\n\n── NOW ──\n\n- [ ] b  ^bbb\n      body\n";
        assert!(!lints(src).contains(&"false-certainty".into()));
    }

    #[test]
    fn a_result_ahead_of_now_is_an_error() {
        let src = "# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n      body\n      → done already\n";
        let out = check(&parse(src).unwrap(), &Config::default());
        let f = out.iter().find(|f| f.lint == "result-ahead").expect("no lint");
        assert_eq!(f.severity, Severity::Error);
    }

    #[test]
    fn a_parent_behind_now_with_open_children_is_an_orphan_parent() {
        let src = "# T\n\n- [x] a  ^aaa\n      - [ ] unfinished\n\n── NOW ──\n\n- [ ] b  ^bbb\n      body\n";
        assert!(lints(src).contains(&"orphan-parent".into()));
    }

    #[test]
    fn children_carrying_status_suggest_they_belong_on_the_line() {
        let src = "# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n      body\n      - [ ] one @blocked(keys)\n";
        assert!(lints(src).contains(&"independent-children".into()));
    }

    #[test]
    fn a_clean_line_produces_no_findings() {
        let src = "# T\n\n- [x] a  ^aaa\n      → shipped\n\n── NOW ──\n\n- [ ] b  ^bbb\n      why this matters\n";
        assert!(check(&parse(src).unwrap(), &Config::default()).is_empty());
    }

    #[test]
    fn warnings_alone_do_not_count_as_errors() {
        let findings = check(
            &parse("# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n").unwrap(),
            &Config::default(),
        );
        assert!(!findings.is_empty());
        assert!(!has_errors(&findings));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl check`
Expected: FAIL — `cannot find function check in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod check;` to `tl/src/lib.rs`. Prepend to `tl/src/check/mod.rs`:

```rust
use crate::config::Config;
use crate::model::{Entry, ItemState, Line};
use crate::view;

const BUCKET_WORDS: [&str; 7] = [
    "backlog", "someday", "later", "v2", "post-launch", "icebox", "blocked",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub lint: &'static str,
    pub severity: Severity,
    pub subject: String,
    pub message: String,
}

pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

pub fn check(line: &Line, cfg: &Config) -> Vec<Finding> {
    let mut out = Vec::new();
    let win = view::window(line, cfg);
    let now = line.now_index();

    for (i, entry) in line.entries.iter().enumerate() {
        match entry {
            Entry::Marker(m) => {
                let label = m.label.to_lowercase();
                let is_bucket = BUCKET_WORDS.iter().any(|w| label.contains(w));
                let allowed = cfg
                    .allow_markers
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(&m.label));
                if is_bucket && !allowed {
                    out.push(Finding {
                        lint: "bucket",
                        severity: Severity::Warning,
                        subject: m.label.clone(),
                        message: "reads as a bucket; if it should happen later, put it later"
                            .into(),
                    });
                }
            }
            Entry::Item(item) => {
                let behind = i < now;
                let in_window = i >= win.start && i < win.end;

                if !behind && !item.result.is_empty() {
                    out.push(Finding {
                        lint: "result-ahead",
                        severity: Severity::Error,
                        subject: item.id.0.clone(),
                        message: "outcomes belong to history; move it behind Now first".into(),
                    });
                }

                if !behind && in_window && !item.is_sharpened() {
                    out.push(Finding {
                        lint: "unsharpened",
                        severity: Severity::Warning,
                        subject: item.id.0.clone(),
                        message: "inside the window but still bare; sharpen before starting"
                            .into(),
                    });
                }

                if !behind && !in_window && item.description.len() > cfg.far_body_lines {
                    out.push(Finding {
                        lint: "false-certainty",
                        severity: Severity::Warning,
                        subject: item.id.0.clone(),
                        message: "detailed but far from Now; distance should mean less detail"
                            .into(),
                    });
                }

                if behind
                    && !matches!(item.state, ItemState::Dropped(_))
                    && item.children.iter().any(|c| !c.done)
                {
                    out.push(Finding {
                        lint: "orphan-parent",
                        severity: Severity::Error,
                        subject: item.id.0.clone(),
                        message: "behind Now with unfinished children".into(),
                    });
                }

                if item
                    .children
                    .iter()
                    .any(|c| c.title.contains("@blocked(") || c.title.contains("@active"))
                {
                    out.push(Finding {
                        lint: "independent-children",
                        severity: Severity::Warning,
                        subject: item.id.0.clone(),
                        message: "children carrying status can be tracked separately; \
                                  they belong on the line"
                            .into(),
                    });
                }
            }
            Entry::Now => {}
        }
    }

    let now_count = line.entries.iter().filter(|e| matches!(e, Entry::Now)).count();
    if now_count != 1 {
        out.push(Finding {
            lint: "no-now",
            severity: Severity::Error,
            subject: "NOW".into(),
            message: format!("expected exactly one NOW marker, found {now_count}"),
        });
    }

    let mut seen = std::collections::HashSet::new();
    for item in line.items() {
        if !seen.insert(item.id.0.clone()) {
            out.push(Finding {
                lint: "duplicate-id",
                severity: Severity::Error,
                subject: item.id.0.clone(),
                message: "two items share this id".into(),
            });
        }
    }

    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl check`
Expected: PASS, 11 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/src/check/ tl/src/lib.rs
git commit -m "feat(check): method lints with warning and error severities"
```

---

### Task 10: CLI read commands

**Files:**
- Create: `tl/src/cli/mod.rs`, `tl/src/cli/render.rs`, `tl/tests/read_commands.rs`
- Modify: `tl/Cargo.toml` (add `clap`, dev-deps `assert_cmd`, `predicates`), `tl/src/main.rs`, `tl/src/lib.rs`, `tl/src/theme/mod.rs` (add `sgr`)
- Test: `tl/tests/read_commands.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–9.
- Produces: `Cli` (clap parser), `cli::run(Cli) -> anyhow::Result<i32>`; `render::entries(&[Entry], &Ctx) -> String`, `Ctx { glyphs: Glyphs, theme: Theme, line: &Line }`; `Theme::sgr(Token) -> String`, `Theme::reset() -> &'static str`.

`--json` output is a stable contract consumed by agents. Field names must not
change without a deliberate decision.

- [ ] **Step 1: Write the failing test**

Create `tl/tests/read_commands.rs`:

```rust
use assert_cmd::Command;
use std::path::Path;

const LINE: &str = "\
# Throughline — Fixture

## Line

- [x] sketch the method  ^k3f
      → ten properties

── NOW ──

- [ ] write the docs  ^q1d
      the full method
- [ ] parse line.md  ^r7e
      grammar and errors

◆ v0.1 ◆

- [ ] build the tui  ^t9a
      ribbon and list
";

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".throughline")).unwrap();
    std::fs::write(dir.path().join(".throughline/line.md"), LINE).unwrap();
    dir
}

fn tl(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("tl").unwrap();
    c.current_dir(dir);
    c
}

#[test]
fn line_prints_every_entry_in_order() {
    let d = fixture();
    let out = tl(d.path()).arg("line").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let order: Vec<&str> = ["sketch the method", "write the docs", "parse line.md", "v0.1", "build the tui"]
        .into_iter()
        .collect();
    let mut last = 0;
    for needle in order {
        let at = text.find(needle).unwrap_or_else(|| panic!("missing {needle}"));
        assert!(at >= last, "{needle} out of order");
        last = at;
    }
}

#[test]
fn non_tty_output_is_ascii_with_no_escape_codes() {
    let d = fixture();
    let out = tl(d.path()).arg("line").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!text.contains('\u{1b}'), "escape codes leaked into piped output");
    assert!(text.is_ascii(), "non-ascii glyphs leaked into piped output");
}

#[test]
fn now_reports_the_next_item_ahead() {
    let d = fixture();
    tl(d.path())
        .arg("now")
        .assert()
        .success()
        .stdout(predicates::str::contains("write the docs"));
}

#[test]
fn window_is_narrower_than_the_whole_line() {
    let d = fixture();
    let out = tl(d.path()).args(["window", "--ahead", "1"]).assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.contains("write the docs"));
    assert!(!text.contains("build the tui"));
}

#[test]
fn slice_returns_only_the_requested_span() {
    let d = fixture();
    let out = tl(d.path()).args(["slice", "^q1d..^r7e"]).assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.contains("write the docs"));
    assert!(text.contains("parse line.md"));
    assert!(!text.contains("sketch the method"));
}

#[test]
fn json_output_carries_the_stable_field_names() {
    let d = fixture();
    let out = tl(d.path()).args(["line", "--json"]).assert().success();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).unwrap();
    let entries = v["entries"].as_array().unwrap();
    assert_eq!(entries[0]["kind"], "item");
    assert_eq!(entries[0]["id"], "k3f");
    assert_eq!(entries[0]["title"], "sketch the method");
    assert_eq!(entries[0]["behind_now"], true);
    assert_eq!(entries[1]["kind"], "now");
    assert_eq!(v["now_index"], 1);
}

#[test]
fn json_marks_future_items_as_ahead() {
    let d = fixture();
    let out = tl(d.path()).args(["now", "--json"]).assert().success();
    let v: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["item"]["behind_now"], false);
}

#[test]
fn a_missing_line_file_fails_with_a_useful_message() {
    let empty = tempfile::tempdir().unwrap();
    tl(empty.path())
        .arg("line")
        .assert()
        .failure()
        .stderr(predicates::str::contains("tl init"));
}

#[test]
fn a_malformed_line_file_reports_the_line_number() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# T\n\n── NOW ──\n\n- [ ] no id here\n",
    )
    .unwrap();
    tl(d.path())
        .arg("line")
        .assert()
        .failure()
        .stderr(predicates::str::contains("line.md:5"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl --test read_commands`
Expected: FAIL — the `tl` binary prints `tl` and ignores all arguments.

- [ ] **Step 3: Write minimal implementation**

Add to `tl/Cargo.toml`:

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
serde_json = "1"
```

Add to `tl/src/theme/mod.rs`, inside `impl Theme`:

```rust
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
```

Add `pub mod cli;` to `tl/src/lib.rs`. Create `tl/src/cli/render.rs`:

```rust
use crate::glyphs::{Glyphs, Role};
use crate::model::{Entry, ItemState, Line};
use crate::theme::{Theme, Token};
use crate::view;
use serde::Serialize;

pub struct Ctx<'a> {
    pub glyphs: Glyphs,
    pub theme: Theme,
    pub line: &'a Line,
}

/// Views name roles and tokens only — never a literal glyph or colour.
pub fn entries(indices: std::ops::Range<usize>, ctx: &Ctx) -> String {
    let now = ctx.line.now_index();
    let mut out = String::new();

    for i in indices {
        let entry = &ctx.line.entries[i];
        let distance = view::distance_from_now(ctx.line, i);
        match entry {
            Entry::Now => {
                out.push_str(&format!(
                    "{}{} NOW {}\n",
                    ctx.theme.sgr(Token::Now),
                    ctx.glyphs.get(Role::Now),
                    ctx.theme.reset()
                ));
            }
            Entry::Marker(m) => {
                out.push_str(&format!(
                    "{}{} {} {}\n",
                    ctx.theme.sgr(Token::Marker),
                    ctx.glyphs.get(Role::Marker),
                    m.label,
                    ctx.theme.reset()
                ));
            }
            Entry::Item(item) => {
                let role = match (&item.state, i < now) {
                    (ItemState::Dropped(_), _) => Role::Dropped,
                    (ItemState::Blocked(_), _) => Role::Blocked,
                    (ItemState::Active, _) => Role::Active,
                    (_, true) => Role::Done,
                    (_, false) => Role::Open,
                };
                let token = match &item.state {
                    ItemState::Dropped(_) => Token::Dropped,
                    ItemState::Blocked(_) => Token::Blocked,
                    _ => ctx.theme.fade(distance),
                };
                out.push_str(&format!(
                    "{}{} {}  ^{}{}\n",
                    ctx.theme.sgr(token),
                    ctx.glyphs.get(role),
                    item.title,
                    item.id.0,
                    ctx.theme.reset()
                ));
            }
        }
    }
    out
}

#[derive(Serialize)]
pub struct JsonEntry {
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub behind_now: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub description: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub result: Vec<String>,
}

pub fn json_entries(indices: std::ops::Range<usize>, line: &Line) -> Vec<JsonEntry> {
    let now = line.now_index();
    indices
        .map(|i| match &line.entries[i] {
            Entry::Now => JsonEntry {
                kind: "now",
                id: None,
                title: None,
                behind_now: false,
                description: vec![],
                result: vec![],
            },
            Entry::Marker(m) => JsonEntry {
                kind: "marker",
                id: None,
                title: Some(m.label.clone()),
                behind_now: i < now,
                description: vec![],
                result: vec![],
            },
            Entry::Item(item) => JsonEntry {
                kind: "item",
                id: Some(item.id.0.clone()),
                title: Some(item.title.clone()),
                behind_now: i < now,
                description: item.description.clone(),
                result: item.result.clone(),
            },
        })
        .collect()
}
```

Create `tl/src/cli/mod.rs`:

```rust
pub mod render;

use crate::config::Config;
use crate::format::io;
use crate::glyphs::{Glyphs, Mode};
use crate::model::{Id, Line, Ref};
use crate::theme::{Depth, Theme, Variant};
use crate::view;
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use render::Ctx;
use std::io::IsTerminal;
use std::path::Path;

#[derive(Parser)]
#[command(name = "tl", about = "Manage a project as one ordered line")]
pub struct Cli {
    #[arg(long, global = true)]
    pub glyphs: Option<String>,
    #[arg(long, global = true)]
    pub theme: Option<String>,
    #[arg(long, global = true, default_value = "auto")]
    pub color: String,
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// The whole line
    Line,
    /// What sits at Now
    Now,
    /// The current attention window
    Window {
        #[arg(long)]
        back: Option<usize>,
        #[arg(long)]
        ahead: Option<usize>,
    },
    /// A span, written `<ref>..<ref>`
    Slice { span: String },
}

fn parse_ref(s: &str) -> Ref {
    match s {
        "now" | "NOW" => Ref::Now,
        other => match other.strip_prefix('^') {
            Some(id) => Ref::Id(Id::new(id)),
            None => Ref::Marker(other.to_string()),
        },
    }
}

fn load() -> Result<(Line, Config, std::path::PathBuf)> {
    let cwd = std::env::current_dir()?;
    let path = io::find_line_file(&cwd)
        .ok_or_else(|| anyhow!("no .throughline/line.md found — run `tl init` to start one"))?;
    let root = path.parent().and_then(|p| p.parent()).unwrap_or(&cwd).to_path_buf();
    let cfg = Config::load(&root);
    Ok((io::read(&path)?, cfg, path))
}

pub fn run(cli: Cli) -> Result<i32> {
    let (line, mut cfg, _path) = load()?;

    let is_tty = std::io::stdout().is_terminal();
    let colour_on = match cli.color.as_str() {
        "always" => true,
        "never" => false,
        _ => is_tty,
    };
    let ctx = Ctx {
        glyphs: Glyphs::for_mode(Mode::resolve(cli.glyphs.as_deref(), &cfg, is_tty)),
        theme: Theme::new(
            Variant::resolve(cli.theme.as_deref(), &cfg),
            if colour_on { Depth::detect(is_tty) } else { Depth::None },
        ),
        line: &line,
    };

    let span = match &cli.command {
        Some(Command::Window { back, ahead }) => {
            if let Some(b) = back { cfg.window_back = *b; }
            if let Some(a) = ahead { cfg.window_ahead = *a; }
            let w = view::window(&line, &cfg);
            w.start..w.end
        }
        Some(Command::Slice { span }) => {
            let (a, b) = span
                .split_once("..")
                .ok_or_else(|| anyhow!("a slice looks like `<ref>..<ref>`"))?;
            let s = view::slice(&line, &parse_ref(a), &parse_ref(b))
                .ok_or_else(|| anyhow!("no entry matches one end of {span}"))?;
            s.start..s.end
        }
        Some(Command::Now) => {
            let item = view::at_now(&line);
            if cli.json {
                let idx = item
                    .and_then(|i| line.index_of(&Ref::Id(i.id.clone())))
                    .map(|i| i..i + 1)
                    .unwrap_or(0..0);
                let entries = render::json_entries(idx, &line);
                println!(
                    "{}",
                    serde_json::json!({ "item": entries.first(), "now_index": line.now_index() })
                );
            } else {
                match item {
                    Some(i) => {
                        let idx = line.index_of(&Ref::Id(i.id.clone())).unwrap();
                        print!("{}", render::entries(idx..idx + 1, &ctx));
                    }
                    None => println!("nothing ahead of Now"),
                }
            }
            return Ok(0);
        }
        _ => 0..line.entries.len(),
    };

    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "entries": render::json_entries(span, &line),
                "now_index": line.now_index(),
            })
        );
    } else {
        print!("{}", render::entries(span, &ctx));
    }
    Ok(0)
}
```

Replace `tl/src/main.rs`:

```rust
use clap::Parser;

fn main() {
    let cli = tl::cli::Cli::parse();
    match tl::cli::run(cli) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("tl: {e}");
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl --test read_commands`
Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/
git commit -m "feat(cli): line, now, window, and slice with --json and non-TTY degradation"
```

---

### Task 11: CLI write commands and `tl check`

**Files:**
- Create: `tl/tests/write_commands.rs`
- Modify: `tl/src/cli/mod.rs`
- Test: `tl/tests/write_commands.rs`

**Interfaces:**
- Consumes: `Command` enum and `run` from Task 10; `Line::insert/move_entry/advance/complete/drop_item` from Task 2; `check`/`has_errors` from Task 9; `io::write_atomic` from Task 5.
- Produces: `Command::{Add, Move, Advance, Done, Drop, Mark, Sharpen, Split, Check, Fmt}` variants.

- [ ] **Step 1: Write the failing test**

Create `tl/tests/write_commands.rs`:

```rust
use assert_cmd::Command;
use std::path::Path;

const LINE: &str = "\
# Fixture

## Line

- [x] sketch  ^k3f

── NOW ──

- [ ] docs  ^q1d
      the full method
- [ ] parse  ^r7e
      grammar

◆ v0.1 ◆

- [ ] tui  ^t9a
      ribbon
";

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".throughline")).unwrap();
    std::fs::write(dir.path().join(".throughline/line.md"), LINE).unwrap();
    dir
}

fn tl(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("tl").unwrap();
    c.current_dir(dir);
    c
}

fn read(dir: &Path) -> String {
    std::fs::read_to_string(dir.path_join()).unwrap()
}

trait P {
    fn path_join(&self) -> std::path::PathBuf;
}
impl P for Path {
    fn path_join(&self) -> std::path::PathBuf {
        self.join(".throughline/line.md")
    }
}

#[test]
fn add_requires_a_placement() {
    let d = fixture();
    tl(d.path()).args(["add", "new work"]).assert().failure();
}

#[test]
fn add_after_places_the_item_immediately_after() {
    let d = fixture();
    tl(d.path())
        .args(["add", "new work", "--after", "^q1d"])
        .assert()
        .success();
    let text = read(d.path());
    let a = text.find("docs").unwrap();
    let b = text.find("new work").unwrap();
    let c = text.find("parse").unwrap();
    assert!(a < b && b < c);
}

#[test]
fn add_after_a_marker_expresses_post_launch_work_as_a_position() {
    let d = fixture();
    tl(d.path())
        .args(["add", "cleanup", "--after", "v0.1"])
        .assert()
        .success();
    let text = read(d.path());
    assert!(text.find("◆ v0.1 ◆").unwrap() < text.find("cleanup").unwrap());
}

#[test]
fn advance_moves_now_and_rewrites_the_checkbox() {
    let d = fixture();
    tl(d.path()).arg("advance").assert().success();
    let text = read(d.path());
    assert!(text.contains("- [x] docs  ^q1d"));
    assert!(text.find("docs").unwrap() < text.find("── NOW ──").unwrap());
}

#[test]
fn advance_records_a_result() {
    let d = fixture();
    tl(d.path())
        .args(["advance", "--result", "shipped it"])
        .assert()
        .success();
    assert!(read(d.path()).contains("→ shipped it"));
}

#[test]
fn done_completes_an_item_out_of_order() {
    let d = fixture();
    tl(d.path()).args(["done", "^t9a"]).assert().success();
    let text = read(d.path());
    assert!(text.contains("- [x] tui  ^t9a"));
    assert!(text.find("tui").unwrap() < text.find("── NOW ──").unwrap());
}

#[test]
fn drop_records_the_reason_and_moves_the_item_behind_now() {
    let d = fixture();
    tl(d.path())
        .args(["drop", "^r7e", "--why", "superseded"])
        .assert()
        .success();
    let text = read(d.path());
    assert!(text.contains("- [-] parse  ^r7e  @dropped(superseded)"));
}

#[test]
fn move_reorders_without_duplicating() {
    let d = fixture();
    tl(d.path())
        .args(["move", "^t9a", "--before", "^q1d"])
        .assert()
        .success();
    let text = read(d.path());
    assert_eq!(text.matches("^t9a").count(), 1);
    assert!(text.find("tui").unwrap() < text.find("docs").unwrap());
}

#[test]
fn mark_places_a_landmark() {
    let d = fixture();
    tl(d.path())
        .args(["mark", "v0.2", "--after", "^t9a"])
        .assert()
        .success();
    assert!(read(d.path()).contains("◆ v0.2 ◆"));
}

#[test]
fn sharpen_adds_a_body() {
    let d = fixture();
    tl(d.path())
        .args(["sharpen", "^t9a", "--body", "ribbon plus window list"])
        .assert()
        .success();
    assert!(read(d.path()).contains("      ribbon plus window list"));
}

#[test]
fn fmt_normalizes_ascii_syntax_and_wrong_checkboxes() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# T\n\n- [ ] past  ^aaa\n\n-- NOW --\n\n<> v1 <>\n\n- [x] future  ^bbb\n",
    )
    .unwrap();
    tl(d.path()).arg("fmt").assert().success();
    let text = read(d.path());
    assert!(text.contains("── NOW ──"));
    assert!(text.contains("◆ v1 ◆"));
    assert!(text.contains("- [x] past  ^aaa"));
    assert!(text.contains("- [ ] future  ^bbb"));
}

#[test]
fn check_exits_zero_when_only_warnings_fire() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# T\n\n── NOW ──\n\n- [ ] bare  ^aaa\n",
    )
    .unwrap();
    tl(d.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicates::str::contains("unsharpened"));
}

#[test]
fn check_exits_non_zero_on_an_error_lint() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n      body\n      → premature\n",
    )
    .unwrap();
    tl(d.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicates::str::contains("result-ahead"));
}

#[test]
fn split_promotes_children_onto_the_line_in_order() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# T\n\n── NOW ──\n\n- [ ] recovery  ^rrr\n      - [ ] token\n      - [ ] email\n- [ ] after  ^zzz\n",
    )
    .unwrap();

    tl(d.path()).args(["split", "^rrr"]).assert().success();
    let text = read(d.path());

    // Children become first-class items, positioned right after the parent.
    assert!(text.find("recovery").unwrap() < text.find("token").unwrap());
    assert!(text.find("token").unwrap() < text.find("email").unwrap());
    assert!(text.find("email").unwrap() < text.find("after").unwrap());
    // And they are no longer nested under the parent.
    assert!(!text.contains("      - [ ] token"));
}

#[test]
fn commit_links_prefer_a_jj_change_id_when_a_jj_repo_is_present() {
    let d = fixture();
    std::fs::create_dir_all(d.path().join(".jj")).unwrap();
    tl(d.path())
        .args(["advance", "--commit", "auto"])
        .assert()
        .success();
    // With `.jj` present and no jj binary reachable in the test environment,
    // `auto` must degrade to no link rather than record a bogus revision.
    let text = read(d.path());
    assert!(!text.contains("@commit(auto)"), "the literal 'auto' was recorded");
}

#[test]
fn an_explicit_commit_value_is_recorded_verbatim() {
    let d = fixture();
    tl(d.path())
        .args(["advance", "--commit", "88ca65b"])
        .assert()
        .success();
    assert!(read(d.path()).contains("@commit(88ca65b)"));
}

#[test]
fn check_json_lists_findings_with_severities() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# T\n\n── NOW ──\n\n- [ ] bare  ^aaa\n",
    )
    .unwrap();
    let out = tl(d.path()).args(["check", "--json"]).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["findings"][0]["lint"], "unsharpened");
    assert_eq!(v["findings"][0]["severity"], "warning");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl --test write_commands`
Expected: FAIL — `error: unrecognized subcommand 'add'`.

- [ ] **Step 3: Write minimal implementation**

Add to the `Command` enum in `tl/src/cli/mod.rs`:

```rust
    /// Add an item. Placement is required.
    Add {
        title: String,
        #[arg(long, group = "place")]
        after: Option<String>,
        #[arg(long, group = "place")]
        before: Option<String>,
        #[arg(long, group = "place")]
        end: bool,
    },
    /// Reorder — this is replanning
    Move {
        id: String,
        #[arg(long, group = "place")]
        after: Option<String>,
        #[arg(long, group = "place")]
        before: Option<String>,
    },
    /// Move Now forward — this is completion
    Advance {
        id: Option<String>,
        #[arg(long)]
        result: Option<String>,
        #[arg(long)]
        commit: Option<String>,
    },
    /// Complete an item out of order
    Done {
        id: String,
        #[arg(long)]
        result: Option<String>,
        #[arg(long)]
        commit: Option<String>,
    },
    /// The other outcome
    Drop {
        id: String,
        #[arg(long)]
        why: String,
        #[arg(long)]
        result: Option<String>,
    },
    /// Place a landmark
    Mark {
        label: String,
        #[arg(long, group = "place")]
        after: Option<String>,
        #[arg(long, group = "place")]
        before: Option<String>,
    },
    /// Add or replace an item's body
    Sharpen {
        id: String,
        #[arg(long)]
        body: String,
    },
    /// Promote children onto the line
    Split { id: String },
    /// Lint the line against the method
    Check,
    /// Normalize derived content
    Fmt,
```

Add helpers and dispatch to `tl/src/cli/mod.rs`. Insert before `pub fn run`:

```rust
use crate::check::{self, Severity};
use crate::model::{Child, Entry, Item, ItemState, Marker, Position};

fn placement(
    after: &Option<String>,
    before: &Option<String>,
    end: bool,
) -> Result<Position> {
    match (after, before, end) {
        (Some(a), _, _) => Ok(Position::After(parse_ref(a))),
        (_, Some(b), _) => Ok(Position::Before(parse_ref(b))),
        (_, _, true) => Ok(Position::End),
        _ => Err(anyhow!(
            "placement is required: pass --after, --before, or --end. \
             Choosing where work goes is the thinking the method asks for."
        )),
    }
}

/// Ids are short, stable, and derived from content so two runs never collide.
fn fresh_id(line: &Line, seed: &str) -> Id {
    let mut hash: u64 = 1469598103934665603;
    for b in seed.bytes().chain(line.entries.len().to_le_bytes()) {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
    for attempt in 0..64u64 {
        let mut n = hash.wrapping_add(attempt);
        let mut s = String::new();
        for _ in 0..3 {
            s.push(alphabet[(n % 36) as usize] as char);
            n /= 36;
        }
        if line.item(&Id::new(s.clone())).is_none() {
            return Id::new(s);
        }
    }
    Id::new(format!("x{}", line.entries.len()))
}

/// Spec 4.4: a git SHA stops resolving the moment the commit is rebased or
/// amended, so a jj change ID is preferred wherever one is available. `--commit
/// auto` asks the repo; anything else is recorded verbatim.
fn resolve_commit(spec: &str, root: &Path) -> Option<String> {
    if spec != "auto" {
        return Some(spec.to_string());
    }
    let run = |program: &str, args: &[&str]| -> Option<String> {
        let out = std::process::Command::new(program)
            .args(args)
            .current_dir(root)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    };
    if root.join(".jj").is_dir() {
        // Change IDs survive rewriting; commit IDs do not.
        if let Some(id) = run("jj", &["log", "-r", "@", "--no-graph", "-T", "change_id.short()"]) {
            return Some(id);
        }
        // A .jj directory with no usable jj binary records nothing rather than
        // a link that was never valid.
        return None;
    }
    run("git", &["rev-parse", "--short", "HEAD"])
}

fn set_outcome(line: &mut Line, id: &Id, result: Option<String>, commit: Option<String>) {
    if let Some(idx) = line.index_of(&Ref::Id(id.clone())) {
        if let Entry::Item(item) = &mut line.entries[idx] {
            if let Some(r) = result {
                item.result = r.lines().map(str::to_string).collect();
            }
            if let Some(c) = commit {
                item.commit = Some(c);
            }
        }
    }
}
```

Insert this block into `run`, immediately after `let (line, mut cfg, _path) = load()?;` — change that binding to `let (mut line, mut cfg, path) = load()?;` first:

```rust
    // Write commands mutate and persist, then return.
    match &cli.command {
        Some(Command::Add { title, after, before, end }) => {
            let id = fresh_id(&line, title);
            line.insert(
                Entry::Item(Item::new(id, title.clone())),
                &placement(after, before, *end)?,
            )?;
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Move { id, after, before }) => {
            line.move_entry(
                &parse_ref(id),
                &placement(after, before, false)?,
            )?;
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Advance { id, result, commit }) => {
            let root = path.parent().and_then(|p| p.parent()).unwrap_or(Path::new("."));
            let rev = commit.as_deref().and_then(|c| resolve_commit(c, root));
            let target = id.as_ref().map(|s| parse_ref(s));
            let passed = line.advance(target.as_ref())?;
            if let Some(last) = passed.last() {
                set_outcome(&mut line, last, result.clone(), rev);
            }
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Done { id, result, commit }) => {
            let root = path.parent().and_then(|p| p.parent()).unwrap_or(Path::new("."));
            let rev = commit.as_deref().and_then(|c| resolve_commit(c, root));
            let id = Id::new(id.trim_start_matches('^'));
            line.complete(&id)?;
            set_outcome(&mut line, &id, result.clone(), rev);
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Drop { id, why, result }) => {
            let id = Id::new(id.trim_start_matches('^'));
            line.drop_item(&id, why.clone())?;
            set_outcome(&mut line, &id, result.clone(), None);
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Mark { label, after, before }) => {
            line.insert(
                Entry::Marker(Marker { label: label.clone() }),
                &placement(after, before, false)?,
            )?;
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Sharpen { id, body }) => {
            let idx = line
                .index_of(&parse_ref(id))
                .ok_or_else(|| anyhow!("no item {id}"))?;
            if let Entry::Item(item) = &mut line.entries[idx] {
                item.description = body.lines().map(str::to_string).collect();
            }
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Split { id }) => {
            let idx = line
                .index_of(&parse_ref(id))
                .ok_or_else(|| anyhow!("no item {id}"))?;
            let children: Vec<Child> = match &mut line.entries[idx] {
                Entry::Item(item) => std::mem::take(&mut item.children),
                _ => return Err(anyhow!("{id} is not an item")),
            };
            let anchor = parse_ref(id);
            for child in children.into_iter().rev() {
                let cid = fresh_id(&line, &child.title);
                let mut item = Item::new(cid, child.title);
                if child.done {
                    item.state = ItemState::Plain;
                }
                line.insert(Entry::Item(item), &Position::After(anchor.clone()))?;
            }
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Fmt) => {
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Check) => {
            let findings = check::check(&line, &cfg);
            if cli.json {
                let rows: Vec<_> = findings
                    .iter()
                    .map(|f| {
                        serde_json::json!({
                            "lint": f.lint,
                            "severity": match f.severity {
                                Severity::Warning => "warning",
                                Severity::Error => "error",
                            },
                            "subject": f.subject,
                            "message": f.message,
                        })
                    })
                    .collect();
                println!("{}", serde_json::json!({ "findings": rows }));
            } else {
                for f in &findings {
                    let tag = match f.severity {
                        Severity::Warning => "warning",
                        Severity::Error => "error",
                    };
                    println!("{tag}: {} [{}] {}", f.subject, f.lint, f.message);
                }
                if findings.is_empty() {
                    println!("the line is clean");
                }
            }
            return Ok(if check::has_errors(&findings) { 1 } else { 0 });
        }
        _ => {}
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl --test write_commands`
Expected: PASS, 17 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/
git commit -m "feat(cli): add, move, advance, done, drop, mark, sharpen, split, fmt, check"
```

---

### Task 12: The ribbon

**Files:**
- Create: `tl/src/tui/mod.rs`, `tl/src/tui/ribbon.rs`
- Modify: `tl/src/lib.rs` (add `pub mod tui;`), `tl/Cargo.toml` (dev-dep `insta`)
- Test: inline `#[cfg(test)]` in `tl/src/tui/ribbon.rs`

**Interfaces:**
- Consumes: `Line`, `Entry`, `ItemState`; `Span` from Task 6; `Glyphs`, `Role` from Task 7; `Token` from Task 8.
- Produces: `Segment { pub text: String, pub token: Token }`, `ribbon::build(&Line, Span, &Glyphs, usize) -> Vec<Segment>`, `ribbon::plain(&[Segment]) -> String`.

The ribbon is built as pure data and only then handed to ratatui. That keeps the
whole-project view testable without a terminal, and keeps the widget free of
layout logic.

- [ ] **Step 1: Write the failing test**

Create `tl/src/tui/ribbon.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;
    use crate::glyphs::{Glyphs, Mode};
    use crate::view;

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl ribbon`
Expected: FAIL — `cannot find function build in this scope`.

- [ ] **Step 3: Write minimal implementation**

Add to `tl/Cargo.toml` under `[dev-dependencies]`:

```toml
insta = "1"
```

Create `tl/src/tui/mod.rs`:

```rust
pub mod ribbon;
```

Add `pub mod tui;` to `tl/src/lib.rs`. Prepend to `tl/src/tui/ribbon.rs`:

```rust
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

fn glyph_for(entry: &Entry, behind_now: bool, g: &Glyphs) -> (String, Role) {
    match entry {
        Entry::Now => (g.get(Role::Now).to_string(), Role::Now),
        Entry::Marker(_) => (g.get(Role::Marker).to_string(), Role::Marker),
        Entry::Item(item) => {
            let role = match (&item.state, behind_now) {
                (ItemState::Dropped(_), _) => Role::Dropped,
                (ItemState::Blocked(_), _) => Role::Blocked,
                (ItemState::Active, _) => Role::Active,
                (_, true) => Role::Done,
                (_, false) => Role::Open,
            };
            (g.get(role).to_string(), role)
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

/// Build the whole-project ribbon. `window` is bracketed; when the line is
/// wider than `width`, entries are dropped from the ends inward so that Now
/// always survives.
pub fn build(line: &Line, window: Span, g: &Glyphs, width: usize) -> Vec<Segment> {
    let now = line.now_index();
    let mut visible: Vec<usize> = (0..line.entries.len()).collect();
    let mut elided_left = false;
    let mut elided_right = false;

    // Each entry costs its glyph plus one rule; brackets cost two more.
    let cost = |ids: &[usize]| -> usize {
        ids.iter()
            .map(|&i| {
                glyph_for(&line.entries[i], i < now, g).0.chars().count() + 1
            })
            .sum::<usize>()
            + 4
    };

    while cost(&visible) > width && visible.len() > 1 {
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

    let mut out = Vec::new();
    if elided_left {
        out.push(Segment { text: "...".into(), token: Token::Muted });
    }
    for (n, &i) in visible.iter().enumerate() {
        if i == window.start {
            out.push(Segment {
                text: g.get(Role::WindowLeft).to_string(),
                token: Token::Window,
            });
        }
        let (text, _) = glyph_for(&line.entries[i], i < now, g);
        out.push(Segment { text, token: token_for(line, i, &line.entries[i]) });
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
    if elided_right {
        out.push(Segment { text: "...".into(), token: Token::Muted });
    }
    out.push(Segment { text: g.get(Role::Arrow).to_string(), token: Token::Muted });
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl ribbon` then `cargo insta accept` to record the snapshot, then re-run.
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/src/tui/ tl/src/lib.rs tl/Cargo.toml tl/src/snapshots/
git commit -m "feat(tui): ribbon built as pure segments with window bracket and elision"
```

---

### Task 13: Window list, app state, and keymap

**Files:**
- Create: `tl/src/tui/list.rs`, `tl/src/tui/app.rs`
- Modify: `tl/src/tui/mod.rs`, `tl/src/cli/mod.rs` (launch TUI when no subcommand)
- Test: inline `#[cfg(test)]` in `tl/src/tui/app.rs` and `tl/src/tui/list.rs`

**Interfaces:**
- Consumes: `ribbon::build`/`Segment` from Task 12; `view::window` from Task 6; model ops from Task 2.
- Produces: `App { pub line: Line, pub cursor: usize, pub cfg: Config, pub dirty: bool }`, `App::new(Line, Config) -> App`, `App::on_key(KeyCode) -> Action`, `Action { None, Quit, Save, AddAfterCursor, Sharpen }`, `list::build(&App, &Glyphs) -> Vec<Vec<Segment>>`, `draw(&mut Frame, &App, &Glyphs, &Theme)`.

The cursor is a view position and is deliberately separate from Now, which is
data (spec 3.3). Moving the cursor never writes.

- [ ] **Step 1: Write the failing test**

Create `tl/src/tui/app.rs`:

```rust
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
        let before = a.variant;
        a.on_key(KeyCode::Char('t'));
        assert_ne!(a.variant, before);
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
        assert!(a.line.entries.iter().any(|e| matches!(
            e,
            crate::model::Entry::Marker(m) if m.label == "v0.2"
        )));
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
```

Create `tl/src/tui/list.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;
    use crate::glyphs::{Glyphs, Mode};
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
    fn each_window_entry_produces_a_row() {
        let rows = build(&app(), &Glyphs::for_mode(Mode::Ascii));
        assert_eq!(rows.len(), 4);
    }

    #[test]
    fn rows_carry_readable_titles_unlike_the_ribbon() {
        let rows = build(&app(), &Glyphs::for_mode(Mode::Ascii));
        let text: String = rows.iter().map(|r| plain(r)).collect();
        assert!(text.contains("why b matters"));
        assert!(text.contains('b'));
    }

    #[test]
    fn the_cursor_row_is_marked() {
        let mut a = app();
        a.cursor = 2;
        let rows = build(&a, &Glyphs::for_mode(Mode::Ascii));
        assert!(rows[2].iter().any(|s| s.token == Token::Cursor));
    }

    #[test]
    fn snapshot_of_the_window_list() {
        let rows = build(&app(), &Glyphs::for_mode(Mode::Ascii));
        let text: String = rows.iter().map(|r| format!("{}\n", plain(r))).collect();
        insta::assert_snapshot!(text);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl tui`
Expected: FAIL — `cannot find type App in this scope`.

- [ ] **Step 3: Write minimal implementation**

Update `tl/src/tui/mod.rs`:

```rust
pub mod app;
pub mod list;
pub mod ribbon;

use crate::config::Config;
use crate::glyphs::{Glyphs, Mode};
// `ratatui::prelude::*` also exports `Line`, so alias ours to keep them apart.
use crate::model::Line as ProjectLine;
use crate::theme::{Depth, Theme};
use crate::view;
use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::path::Path;

pub fn launch(line: ProjectLine, cfg: Config, path: &Path, mode: Mode, theme_variant: crate::theme::Variant) -> Result<()> {
    enable_raw_mode()?;
    let mut out = std::io::stdout();
    crossterm::execute!(out, EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;

    let mut app = app::App::new(line, cfg);
    app.variant = theme_variant;
    let glyphs = Glyphs::for_mode(mode);

    loop {
        let theme = Theme::new(app.variant, Depth::True);
        term.draw(|f| draw(f, &app, &glyphs, &theme))?;
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            // While a prompt is open, keys feed the buffer instead of the keymap.
            if app.prompt.is_some() {
                match k.code {
                    crossterm::event::KeyCode::Enter => app.commit_prompt(),
                    crossterm::event::KeyCode::Esc => {
                        app.prompt = None;
                        app.buffer.clear();
                    }
                    crossterm::event::KeyCode::Backspace => {
                        app.buffer.pop();
                    }
                    crossterm::event::KeyCode::Char(c) => app.buffer.push(c),
                    _ => {}
                }
                continue;
            }
            if app::Action::Quit == app.on_key(k.code) {
                break;
            }
        }
        if app.dirty {
            crate::format::io::write_atomic(path, &app.line)?;
            app.dirty = false;
        }
    }

    disable_raw_mode()?;
    crossterm::execute!(term.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn to_line<'a>(segments: &[ribbon::Segment], theme: &Theme) -> Line<'a> {
    Line::from(
        segments
            .iter()
            .map(|s| Span::styled(s.text.clone(), theme.style(s.token)))
            .collect::<Vec<_>>(),
    )
}

pub fn draw(f: &mut Frame, app: &app::App, glyphs: &Glyphs, theme: &Theme) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(f.area());
    let window = view::window(&app.line, &app.cfg);
    let segs = ribbon::build(&app.line, window, glyphs, chunks[0].width as usize);
    f.render_widget(Paragraph::new(to_line(&segs, theme)), chunks[0]);

    let rows: Vec<Line> = list::build(app, glyphs)
        .iter()
        .map(|r| to_line(r, theme))
        .collect();
    f.render_widget(Paragraph::new(rows), chunks[1]);
}
```

Prepend to `tl/src/tui/app.rs`:

```rust
use crate::config::Config;
use crate::model::{Entry, Id, Item, Line, Marker, Position, Ref};
use crate::theme::Variant;
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
    pub variant: Variant,
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
            variant: Variant::Dark,
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
            KeyCode::Char('t') => {
                self.variant = match self.variant {
                    Variant::Dark => Variant::Light,
                    Variant::Light => Variant::Dark,
                }
            }
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
                    if self.line.drop_item(&id, "dropped in the TUI".into()).is_ok() {
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
        let Some(prompt) = self.prompt.take() else { return };
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
```

Prepend to `tl/src/tui/list.rs`:

```rust
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
                for d in &item.description {
                    rows.push(row.clone());
                    row = vec![Segment { text: format!("      {d}"), token: Token::Muted }];
                }
            }
        }
        rows.push(row);
    }
    rows
}
```

Finally, in `tl/src/cli/mod.rs`, launch the TUI when no subcommand is given.
Replace the `_ => 0..line.entries.len(),` arm of the `span` match with:

```rust
        None => {
            let mode = Mode::resolve(cli.glyphs.as_deref(), &cfg, is_tty);
            let variant = Variant::resolve(cli.theme.as_deref(), &cfg);
            crate::tui::launch(line, cfg, &path, mode, variant)?;
            return Ok(0);
        }
        _ => 0..line.entries.len(),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl tui` then `cargo insta accept`, then re-run.
Expected: PASS, 20 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/
git commit -m "feat(tui): window list, app state, keymap, and the two-zoom-level screen"
```

---

### Task 14: `tl init`, `tl doctor`, `tl plan`

**Files:**
- Create: `tl/src/cli/init.rs`, `tl/tests/init_commands.rs`
- Modify: `tl/src/cli/mod.rs`
- Test: `tl/tests/init_commands.rs`

**Interfaces:**
- Consumes: `io::write_atomic` from Task 5; `Glyphs`, `Mode` from Task 7.
- Produces: `Command::{Init, Doctor, Plan}`; `init::scaffold(&Path) -> Result<()>`, `init::sample_rows() -> String`, `init::from_plan(&Path, &Path) -> Result<Line>`.

`tl init` writes the agent stanza; that is what makes the vocabulary available
to Claude and Codex without being told each session (spec 6.2).

- [ ] **Step 1: Write the failing test**

Create `tl/tests/init_commands.rs`:

```rust
use assert_cmd::Command;
use std::path::Path;

fn tl(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("tl").unwrap();
    c.current_dir(dir);
    c
}

#[test]
fn init_creates_a_parseable_line_and_the_agent_stanza() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();

    assert!(d.path().join(".throughline/line.md").is_file());
    assert!(d.path().join("THROUGHLINE.md").is_file());
    assert!(d.path().join("AGENTS.md").is_file());

    // The freshly created line must be readable by the tool itself.
    tl(d.path()).arg("line").assert().success();
}

#[test]
fn init_is_idempotent_and_never_clobbers_an_existing_line() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# Mine\n\n── NOW ──\n\n- [ ] keep me  ^kkk\n",
    )
    .unwrap();
    tl(d.path()).arg("init").assert().success();

    let text = std::fs::read_to_string(d.path().join(".throughline/line.md")).unwrap();
    assert!(text.contains("keep me"));
}

#[test]
fn the_agent_stanza_names_the_commands_and_the_discipline() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    let text = std::fs::read_to_string(d.path().join("AGENTS.md")).unwrap();
    for needle in ["tl window", "tl add", "tl advance", "tl check", "behind Now"] {
        assert!(text.contains(needle), "AGENTS.md is missing {needle}");
    }
}

#[test]
fn doctor_prints_all_three_glyph_modes_for_comparison() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    let out = tl(d.path()).arg("doctor").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.contains("nerdfont"));
    assert!(text.contains("unicode"));
    assert!(text.contains("ascii"));
}

#[test]
fn plan_seeds_a_line_from_a_markdown_document() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    std::fs::write(
        d.path().join("plan.md"),
        "# A Plan\n\n### Task 1: First thing\n\nbody\n\n### Task 2: Second thing\n\nbody\n",
    )
    .unwrap();

    tl(d.path()).args(["plan", "plan.md"]).assert().success();
    let text = std::fs::read_to_string(d.path().join(".throughline/line.md")).unwrap();
    assert!(text.contains("First thing"));
    assert!(text.contains("Second thing"));
    assert!(text.find("First thing").unwrap() < text.find("Second thing").unwrap());
}

#[test]
fn plan_places_seeded_work_ahead_of_now() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    std::fs::write(d.path().join("p.md"), "### Task 1: Only thing\n").unwrap();
    tl(d.path()).args(["plan", "p.md"]).assert().success();

    let text = std::fs::read_to_string(d.path().join(".throughline/line.md")).unwrap();
    assert!(text.find("── NOW ──").unwrap() < text.find("Only thing").unwrap());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl --test init_commands`
Expected: FAIL — `error: unrecognized subcommand 'init'`.

- [ ] **Step 3: Write minimal implementation**

Add to the `Command` enum in `tl/src/cli/mod.rs`:

```rust
    /// Create .throughline/, a method summary, and the agent stanza
    Init,
    /// Re-run glyph and theme capability detection
    Doctor,
    /// Seed a line from a plan document
    Plan { file: std::path::PathBuf },
```

Create `tl/src/cli/init.rs`:

```rust
use crate::format::io;
use crate::glyphs::{Glyphs, Mode, Role};
use crate::model::{Entry, Id, Item, Line};
use anyhow::Result;
use std::path::Path;

const THROUGHLINE_MD: &str = "\
# Throughline

This project is managed as one ordered line running from past, through Now,
into the future. `.throughline/line.md` is that line.

- **Status is position.** Done is not a flag, it is a location: behind Now.
- **Markers are landmarks, not buckets.** \"After launch\" is a place on the
  line, not a container.
- **The Window is a view.** Now is stored; the window is wherever you look.
- **Planning is progressive.** Detail decreases with distance from Now.

Vocabulary: the Line, Now, the Window. Move it forward on the Line. That is
outside the current Window. Do not create a post-launch bucket — put it after
launch.
";

const AGENTS_MD: &str = "\
## Throughline

Project work lives in `.throughline/line.md` as one ordered line. Read it in
full for complete context: it is simultaneously the plan, the queue, and the
record.

- `tl window --json` — what is currently in focus
- `tl now --json` — the next item ahead of Now
- `tl add \"title\" --after <ref>` — placement is required; there is no backlog
- `tl advance --result \"what happened\"` — completion moves Now forward
- `tl check` — lint the line against the method before finishing

Completion is position, not state: an item is done when it sits **behind Now**.
Record what actually happened with `--result`; that is where the project keeps
what it learned.
";

pub fn scaffold(root: &Path) -> Result<()> {
    let line_path = root.join(".throughline/line.md");
    if !line_path.exists() {
        let line = Line {
            title: "Throughline".into(),
            entries: vec![
                Entry::Now,
                Entry::Item({
                    let mut i = Item::new(Id::new("aaa"), "decide what comes first");
                    i.description = vec!["Placement is the thinking; put it where it belongs.".into()];
                    i
                }),
            ],
        };
        io::write_atomic(&line_path, &line)?;
    }
    for (name, body) in [("THROUGHLINE.md", THROUGHLINE_MD), ("AGENTS.md", AGENTS_MD)] {
        let p = root.join(name);
        if !p.exists() {
            std::fs::write(p, body)?;
        }
    }
    Ok(())
}

/// The same row rendered three ways, so the user picks by looking rather than
/// by guessing whether a Nerd Font is installed (spec 7.1).
pub fn sample_rows() -> String {
    let mut out = String::new();
    for (name, mode) in [
        ("nerdfont", Mode::NerdFont),
        ("unicode", Mode::Unicode),
        ("ascii", Mode::Ascii),
    ] {
        let g = Glyphs::for_mode(mode);
        out.push_str(&format!(
            "{:>9}  {} {} {} {} {} {} {}\n",
            name,
            g.get(Role::Done),
            g.get(Role::Done),
            g.get(Role::Now),
            g.get(Role::Open),
            g.get(Role::Marker),
            g.get(Role::Open),
            g.get(Role::Arrow),
        ));
    }
    out.push_str("\nSet your choice with: tl --glyphs <mode>, TL_GLYPHS, or\n");
    out.push_str(".throughline/config.toml -> glyphs = \"<mode>\"\n");
    out
}

/// Seed items from `### Task N: Title` headings, in document order.
pub fn from_plan(plan: &Path, line: &mut Line) -> Result<()> {
    let text = std::fs::read_to_string(plan)?;
    let mut anchor = crate::model::Ref::Now;
    for raw in text.lines() {
        let Some(rest) = raw.strip_prefix("### ") else { continue };
        let title = match rest.split_once(':') {
            Some((_, t)) => t.trim(),
            None => rest.trim(),
        };
        if title.is_empty() {
            continue;
        }
        let id = super::fresh_id(line, title);
        line.insert(
            Entry::Item(Item::new(id.clone(), title)),
            &crate::model::Position::After(anchor.clone()),
        )?;
        anchor = crate::model::Ref::Id(id);
    }
    Ok(())
}
```

In `tl/src/cli/mod.rs`, add `pub mod init;`, make `fresh_id` `pub(crate)`, and
handle the three commands *before* `load()` (since `init` runs where no line
exists yet). Insert at the very top of `run`:

```rust
    if let Some(Command::Init) = cli.command {
        let root = std::env::current_dir()?;
        init::scaffold(&root)?;
        println!("initialised .throughline/ — run `tl` to open the line");
        return Ok(0);
    }
    if let Some(Command::Doctor) = cli.command {
        print!("{}", init::sample_rows());
        return Ok(0);
    }
```

And in the write-command match block, add:

```rust
        Some(Command::Plan { file }) => {
            init::from_plan(file, &mut line)?;
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl --test init_commands`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add tl/
git commit -m "feat(cli): init scaffolding, doctor glyph picker, and plan seeding"
```

---

### Task 15: Generated diagrams

**Files:**
- Create: `tl/src/diagrams.rs`, `tl/tests/diagrams.rs`
- Modify: `tl/src/lib.rs`, `tl/src/cli/mod.rs`
- Test: `tl/tests/diagrams.rs`

**Interfaces:**
- Consumes: `parse` from Task 3; `ribbon::build`/`plain` from Task 12; `Glyphs`, `Mode` from Task 7.
- Produces: `diagrams::NAMES: [&str; 7]`, `diagrams::render(name: &str) -> Option<String>`; `Command::Diagram { name: Option<String> }`.

Spec 9.3: the seven line-shaped diagrams are generated so `docs/method.md`
cannot drift from the tool's real output. The test that enforces this asserts
each rendered diagram appears verbatim in the document.

- [ ] **Step 1: Write the failing test**

Create `tl/tests/diagrams.rs`:

```rust
use assert_cmd::Command;

#[test]
fn every_named_diagram_renders_something() {
    for name in tl::diagrams::NAMES {
        let out = tl::diagrams::render(name)
            .unwrap_or_else(|| panic!("{name} rendered nothing"));
        assert!(out.contains('│') || out.contains('●') || out.contains('○'),
                "{name} does not look like a line: {out}");
    }
}

#[test]
fn diagrams_use_the_unicode_glyph_set_not_ascii() {
    let out = tl::diagrams::render("the-line").unwrap();
    assert!(!out.contains("[x]"), "diagrams must be unicode, not ascii (spec 9.3)");
}

#[test]
fn an_unknown_diagram_name_is_none() {
    assert!(tl::diagrams::render("nope").is_none());
}

#[test]
fn the_cli_can_print_a_diagram_by_name() {
    Command::cargo_bin("tl")
        .unwrap()
        .args(["diagram", "the-line"])
        .assert()
        .success();
}

#[test]
fn every_generated_diagram_appears_verbatim_in_the_method_document() {
    let doc = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/method.md"),
    )
    .expect("docs/method.md must exist");

    for name in tl::diagrams::NAMES {
        let rendered = tl::diagrams::render(name).unwrap();
        assert!(
            doc.contains(rendered.trim_end()),
            "docs/method.md has drifted from `tl diagram {name}`.\n\
             Regenerate with: tl diagram --all"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p tl --test diagrams`
Expected: FAIL — `could not find diagrams in tl`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod diagrams;` to `tl/src/lib.rs`. Create `tl/src/diagrams.rs`:

```rust
use crate::config::Config;
use crate::format::parse;
use crate::glyphs::{Glyphs, Mode};
use crate::model::{Line, Ref};
use crate::tui::ribbon::{build, plain};
use crate::view;

/// The seven line-shaped diagrams from spec 9.2.
pub const NAMES: [&str; 7] = [
    "the-line",
    "sliding-window",
    "selected-slice",
    "landmark-not-bucket",
    "history-months",
    "markers-after-launch",
    "window-is-a-view",
];

fn fixture(past: &[&str], future: &[&str], markers: &[(usize, &str)]) -> Line {
    let mut src = String::from("# Diagram\n\n");
    for (n, t) in past.iter().enumerate() {
        src.push_str(&format!("- [x] {t}  ^p{n}\n"));
    }
    src.push_str("\n── NOW ──\n\n");
    for (n, t) in future.iter().enumerate() {
        if let Some((_, label)) = markers.iter().find(|(at, _)| *at == n) {
            src.push_str(&format!("\n◆ {label} ◆\n\n"));
        }
        src.push_str(&format!("- [ ] {t}  ^f{n}\n"));
    }
    parse(&src).unwrap()
}

fn ribbon(line: &Line, cfg: &Config) -> String {
    let w = view::window(line, cfg);
    plain(&build(line, w, &Glyphs::for_mode(Mode::Unicode), 200))
}

pub fn render(name: &str) -> Option<String> {
    let cfg = Config::default();
    Some(match name {
        "the-line" => {
            let l = fixture(&["a", "b", "c", "d"], &["e", "f", "g", "h"], &[]);
            format!(
                "PAST                    NOW                   FUTURE\n{}\n",
                ribbon(&l, &Config { window_back: 0, window_ahead: 0, ..cfg })
            )
        }
        "sliding-window" => {
            let l = fixture(&["a", "b", "c", "d"], &["e", "f", "g", "h", "i"], &[]);
            format!("{}\n            the window moves forward\n", ribbon(&l, &cfg))
        }
        "selected-slice" => {
            let l = fixture(&["a", "b", "c", "d", "e"], &["f", "g"], &[]);
            let s = view::slice(&l, &Ref::Id(crate::model::Id::new("p1")), &Ref::Id(crate::model::Id::new("p3")))?;
            format!(
                "{}\n         a slice answers: what happened here?\n",
                plain(&build(&l, s, &Glyphs::for_mode(Mode::Unicode), 200))
            )
        }
        "landmark-not-bucket" | "markers-after-launch" => {
            let l = fixture(&["a", "b"], &["c", "d", "e", "f"], &[(2, "launch")]);
            format!(
                "{}\n                        work after launch is a PLACE,\n                        not a bucket\n",
                ribbon(&l, &cfg)
            )
        }
        "history-months" => {
            let l = fixture(
                &["discovery", "prototype", "experiment"],
                &["release", "learn"],
                &[],
            );
            format!(
                "JAN         FEB         MAR         APR         MAY\n{}\n",
                ribbon(&l, &Config { window_back: 0, window_ahead: 0, ..cfg })
            )
        }
        "window-is-a-view" => {
            let l = fixture(&["a", "b", "c", "d", "e", "f"], &["g", "h", "i", "j"], &[]);
            let back = view::slice(&l, &Ref::Id(crate::model::Id::new("p0")), &Ref::Id(crate::model::Id::new("p2")))?;
            let fwd = view::slice(&l, &Ref::Id(crate::model::Id::new("f1")), &Ref::Id(crate::model::Id::new("f3")))?;
            format!(
                "reviewing March:\n{}\n\nplanning Q3:\n{}\n\n        one line, two readers, no copies\n",
                plain(&build(&l, back, &Glyphs::for_mode(Mode::Unicode), 200)),
                plain(&build(&l, fwd, &Glyphs::for_mode(Mode::Unicode), 200)),
            )
        }
        _ => return None,
    })
}
```

Add to the `Command` enum in `tl/src/cli/mod.rs`:

```rust
    /// Print a generated method-document diagram
    Diagram {
        name: Option<String>,
        #[arg(long)]
        all: bool,
    },
```

And handle it at the top of `run`, next to `Init` and `Doctor`:

```rust
    if let Some(Command::Diagram { name, all }) = &cli.command {
        if *all {
            for n in crate::diagrams::NAMES {
                println!("<!-- diagram: {n} -->\n```\n{}```\n", crate::diagrams::render(n).unwrap());
            }
        } else if let Some(n) = name {
            match crate::diagrams::render(n) {
                Some(d) => print!("{d}"),
                None => return Err(anyhow!("no diagram named {n}")),
            }
        }
        return Ok(0);
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p tl --test diagrams`
Expected: FAIL on the last test only — `docs/method.md must exist`. That test is
satisfied by Task 16; the other four must pass now.

- [ ] **Step 5: Commit**

```bash
git add tl/
git commit -m "feat(diagrams): generate the seven line-shaped method diagrams"
```

---

### Task 16: `docs/method.md`

**Files:**
- Create: `docs/method.md`
- Test: `tl/tests/diagrams.rs::every_generated_diagram_appears_verbatim_in_the_method_document` (written in Task 15)

**Interfaces:**
- Consumes: `tl diagram --all` from Task 15.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Run the failing test**

Run: `cargo test -p tl --test diagrams every_generated`
Expected: FAIL — `docs/method.md must exist`.

- [ ] **Step 2: Generate the diagram blocks**

```bash
cargo run -p tl -- diagram --all > /tmp/tl-diagrams.md
```

- [ ] **Step 3: Write the document**

Create `docs/method.md` with these sections, in this order. Each numbered
section states one claim and carries the diagram that argues it. Prose is drawn
from spec sections 2 and 3; do not invent new claims.

| § | title | diagram | source |
|---|---|---|---|
| 1 | The project is a line | `the-line` | generated |
| 2 | Work is cyclical, but progress is linear | PDSA circle, then the same cycle unrolled onto time | hand-drawn |
| 3 | The sliding window | `sliding-window` | generated |
| 4 | Any slice tells a story | `selected-slice` | generated |
| 5 | Strict ordering | `A → B → C → D` | hand-drawn |
| 6 | Hierarchy exists, but should be rare | a task tree beside the same work spread on the line | hand-drawn |
| 7 | Avoid temporal buckets | `landmark-not-bucket` | generated |
| 8 | History is first-class | `history-months` | generated |
| 9 | Planning is progressive | detail density fading with distance | hand-drawn |
| 10 | The core model | (none — a list) | — |
| 11 | Status is position | one item crossing Now, before and after | hand-drawn |
| 12 | Markers are landmarks | `markers-after-launch` | generated |
| 13 | The Window is a view | `window-is-a-view` | generated |
| 14 | The central inversion | Kanban's moving cards beside "you are here" | hand-drawn |
| 15 | Using `tl` | the ribbon above the window list | hand-drawn |

Paste the generated blocks from `/tmp/tl-diagrams.md` verbatim into their
sections — including the `<!-- diagram: name -->` comment, which marks them as
generated and tells the next reader not to hand-edit them.

Hand-drawn diagrams use the same unicode glyph vocabulary: `●` `○` `◆` `│` `▶`.
Do not use ascii forms; spec 9.3 reserves those for degraded terminals.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p tl --test diagrams`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add docs/method.md
git commit -m "docs: the Throughline Method, with generated and hand-drawn diagrams"
```

---

### Task 17: Dogfood

**Files:**
- Create: `.throughline/line.md`, `README.md`
- Test: manual, plus `tl check`

**Interfaces:**
- Consumes: the whole binary.
- Produces: nothing.

- [ ] **Step 1: Seed the line from this plan**

```bash
cargo run -p tl -- init
cargo run -p tl -- plan docs/superpowers/plans/2026-08-13-throughline-poc.md
```

- [ ] **Step 2: Record the work already done**

Every task in this plan is finished by the time this runs, so move them behind
Now with the result each one produced. For each of the seventeen seeded items,
run `tl done` with a one-line result. For example:

```bash
cargo run -p tl -- done ^<id> --result "Line, entries, and position queries; 4 tests."
```

- [ ] **Step 3: Add the work that comes next**

```bash
cargo run -p tl -- mark "v0.1 — tl manages its own line" --after now
cargo run -p tl -- add "MCP server over stdio" --end
cargo run -p tl -- add "user-authored themes" --end
```

- [ ] **Step 4: Verify the line passes its own lints**

Run: `cargo run -p tl -- check`
Expected: exit 0. Warnings about unsharpened items are acceptable; any error
lint is a bug in either the line or the tool and must be fixed before commit.

Then open it: `cargo run -p tl` — confirm the ribbon shows the whole project
with the window bracketed, and that `j`/`k`/`n` move without writing.

- [ ] **Step 5: Commit**

```bash
git add .throughline/line.md README.md
git commit -m "chore: manage Throughline in Throughline"
```

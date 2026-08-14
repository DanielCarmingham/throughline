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
Expected: PASS, 8 tests.

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

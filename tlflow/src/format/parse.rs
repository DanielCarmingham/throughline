use crate::model::*;
use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: message.into(),
    }
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
    let mut id = None;
    let mut commit = None;
    let mut state = ItemState::Plain;
    let mut bare = text.to_string();

    // `@blocked(...)`/`@dropped(...)` may contain spaces, so recover them from
    // the raw text rather than from whitespace tokens.
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

    let mut title = Vec::new();
    for tok in bare.split_whitespace() {
        if let Some(rest) = tok.strip_prefix('^') {
            id = Some(Id::new(rest));
        } else if let Some(rest) = tok
            .strip_prefix("@commit(")
            .and_then(|s| s.strip_suffix(')'))
        {
            commit = Some(rest.to_string());
        } else if tok == "@active" {
            state = ItemState::Active;
        } else {
            title.push(tok);
        }
    }

    (title.join(" ").trim().to_string(), id, commit, state)
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
        if indented && trimmed.starts_with("- [") {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Throughline — Test

## Line

- [x] Sketch the practice  ^k3f
      → Ten properties.
        The window idea is the original one.
- [x] Pick the name  ^m2a  @commit(88ca65b)

── NOW ──

- [ ] Write docs/method.md  ^q1d
      The full practice: line, Now, window.
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
        assert_eq!(
            item.description,
            vec!["The full practice: line, Now, window."]
        );
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

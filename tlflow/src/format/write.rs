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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::parse;

    const SAMPLE: &str = "\
# Throughline — Test

## Line

- [x] Sketch the practice  ^k3f
      → Ten properties.
        The window idea is the original one.

── NOW ──

- [ ] Write docs/method.md  ^q1d
      The full practice: line, Now, window.
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

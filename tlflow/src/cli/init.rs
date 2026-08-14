use crate::format::io;
use crate::glyphs::{Glyphs, Mode, Role};
use crate::model::{Entry, Id, Item, Line, Position, Ref};
use anyhow::Result;
use std::path::Path;

const THROUGHLINE_MD: &str = "\
# Throughline

*Learn in cycles. Move in a line.*

A practice of continuous inquiry and forward flow. Work lives on one ordered
line running from past, through Now, into the future. `.throughline/line.md`
is that line.

- **Status is position.** Done is not a flag, it is a location: behind Now.
- **Markers are landmarks, not buckets.** \"After launch\" is a place on the
  line, not a container.
- **The Window is a view.** Now is stored; the window is wherever you look.
- **Planning is progressive.** Detail decreases with distance from Now.
- **Inquiry decides what comes next.** What happened, and what does that change?

Vocabulary: Practice, the Line, Flow, the Window, Inquiry, Cycles, Now. Move it
forward on the Line. That is outside the current Window. Do not create a
post-launch bucket — put it after launch.
";

const AGENTS_MD: &str = "\
## Throughline

Project work lives in `.throughline/line.md` as one ordered line. Read it in
full for complete context: it is simultaneously the plan, the queue, and the
record.

- `tlflow window --json` — what is currently in focus
- `tlflow now --json` — the next item ahead of Now
- `tlflow add \"title\" --after <ref>` — placement is required; there is no backlog
- `tlflow advance --result \"what happened\"` — completion moves Now forward
- `tlflow check` — lint the line against the practice before finishing

Completion is position, not state: an item is done when it sits **behind Now**.
Record what actually happened with `--result`; that is where the practice keeps
what it learned.

Then ask the question that moves things: **what does that change?** Act on the
answer by editing the line ahead — `tlflow move`, `tlflow add`, `tlflow drop`. A result that
changes nothing was not worth recording.
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
                    i.description =
                        vec!["Placement is the thinking; put it where it belongs.".into()];
                    i
                }),
            ],
        };
        io::write_atomic(&line_path, &line)?;
    }
    for (name, body) in [
        ("THROUGHLINE.md", THROUGHLINE_MD),
        ("AGENTS.md", AGENTS_MD),
    ] {
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
pub fn from_plan(plan: &Path, line: &mut Line) -> Result<usize> {
    let text = std::fs::read_to_string(plan)?;
    let mut anchor = Ref::Now;
    let mut added = 0;
    for raw in text.lines() {
        let Some(rest) = raw.strip_prefix("### ") else {
            continue;
        };
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
            &Position::After(anchor.clone()),
        )?;
        anchor = Ref::Id(id);
        added += 1;
    }
    Ok(added)
}

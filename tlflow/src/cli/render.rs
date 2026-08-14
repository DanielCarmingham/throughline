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

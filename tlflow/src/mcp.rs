//! An MCP server over stdio, so an agent gets typed tools instead of parsing
//! CLI output.
//!
//! Every tool re-reads the line from disk before acting. The file is the source
//! of truth (spec 4.1) and a human or another process may have edited it
//! between calls, so holding it in memory would let the two diverge.

use crate::check;
use crate::config::Config;
use crate::format::io;
use crate::model::{Entry, Id, Item, Line, Position, Ref};
use crate::view;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone)]
pub struct Throughline {
    path: PathBuf,
    root: PathBuf,
}

#[derive(Serialize, JsonSchema)]
pub struct EntryOut {
    /// "item", "marker" or "now"
    pub kind: String,
    pub id: Option<String>,
    pub title: Option<String>,
    /// True when the entry sits behind Now, i.e. it is history.
    pub behind_now: bool,
    pub description: Vec<String>,
    pub result: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct LineOut {
    pub entries: Vec<EntryOut>,
    pub now_index: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct ItemOut {
    pub item: Option<EntryOut>,
}

#[derive(Serialize, JsonSchema)]
pub struct FindingOut {
    pub lint: String,
    pub severity: String,
    pub subject: String,
    pub message: String,
}

#[derive(Serialize, JsonSchema)]
pub struct CheckOut {
    pub findings: Vec<FindingOut>,
    pub clean: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct Wrote {
    pub ok: bool,
    /// The question Inquiry asks after every recorded outcome (spec 3.4).
    pub what_does_that_change: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct WindowArgs {
    /// Items to include behind Now. Defaults to the configured window.
    pub back: Option<usize>,
    /// Items to include ahead of Now.
    pub ahead: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AddArgs {
    pub title: String,
    /// Place after this ref: `^id`, a marker label, or `now`.
    pub after: Option<String>,
    /// Place before this ref.
    pub before: Option<String>,
    /// Place at the end of the line.
    pub end: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AdvanceArgs {
    /// Advance past everything up to and including this ref. Omit for the next item.
    pub to: Option<String>,
    /// What actually happened. This is where the practice keeps what it learned.
    pub result: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct MoveArgs {
    pub id: String,
    pub after: Option<String>,
    pub before: Option<String>,
}

fn to_entry(line: &Line, i: usize) -> EntryOut {
    let now = line.now_index();
    match &line.entries[i] {
        Entry::Now => EntryOut {
            kind: "now".into(),
            id: None,
            title: None,
            behind_now: false,
            description: vec![],
            result: vec![],
        },
        Entry::Marker(m) => EntryOut {
            kind: "marker".into(),
            id: None,
            title: Some(m.label.clone()),
            behind_now: i < now,
            description: vec![],
            result: vec![],
        },
        Entry::Item(it) => EntryOut {
            kind: "item".into(),
            id: Some(it.id.0.clone()),
            title: Some(it.title.clone()),
            behind_now: i < now,
            description: it.description.clone(),
            result: it.result.clone(),
        },
    }
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

const INQUIRY: &str = "what does that change? act on it with move, add or drop — a result that \
     changes nothing was not worth recording";

#[tool_router]
impl Throughline {
    pub fn new(path: PathBuf, root: PathBuf) -> Self {
        Throughline { path, root }
    }

    /// Always read fresh: the file is the source of truth and may have been
    /// edited by hand between calls.
    fn load(&self) -> Result<(Line, Config), String> {
        let line = io::read(&self.path).map_err(|e| e.to_string())?;
        Ok((line, Config::load(&self.root)))
    }

    fn save(&self, line: &Line) -> Result<(), String> {
        io::write_atomic(&self.path, line).map_err(|e| e.to_string())
    }

    #[tool(
        description = "The whole line: history behind Now, intention ahead of it. \
                       Read this for complete context."
    )]
    fn line(&self) -> Json<LineOut> {
        match self.load() {
            Ok((l, _)) => Json(LineOut {
                entries: (0..l.entries.len()).map(|i| to_entry(&l, i)).collect(),
                now_index: l.now_index(),
            }),
            Err(_) => Json(LineOut {
                entries: vec![],
                now_index: 0,
            }),
        }
    }

    #[tool(description = "The current attention window — what is in focus around Now.")]
    fn window(&self, Parameters(a): Parameters<WindowArgs>) -> Json<LineOut> {
        let Ok((l, mut cfg)) = self.load() else {
            return Json(LineOut {
                entries: vec![],
                now_index: 0,
            });
        };
        if let Some(b) = a.back {
            cfg.window_back = b;
        }
        if let Some(f) = a.ahead {
            cfg.window_ahead = f;
        }
        let w = view::window(&l, &cfg);
        Json(LineOut {
            entries: (w.start..w.end).map(|i| to_entry(&l, i)).collect(),
            now_index: l.now_index(),
        })
    }

    #[tool(description = "The next item ahead of Now — where work is happening.")]
    fn now(&self) -> Json<ItemOut> {
        let Ok((l, _)) = self.load() else {
            return Json(ItemOut { item: None });
        };
        let item = view::at_now(&l)
            .and_then(|i| l.index_of(&Ref::Id(i.id.clone())))
            .map(|i| to_entry(&l, i));
        Json(ItemOut { item })
    }

    #[tool(
        description = "Add an item. Placement is required — pass after, before or \
                       end. Choosing where work goes is the thinking the practice \
                       asks for; there is no backlog to drop things into."
    )]
    fn add(&self, Parameters(a): Parameters<AddArgs>) -> Json<Wrote> {
        let Ok((mut l, _)) = self.load() else {
            return Json(Wrote {
                ok: false,
                what_does_that_change: None,
            });
        };
        let pos = match (&a.after, &a.before, a.end.unwrap_or(false)) {
            (Some(r), _, _) => Position::After(parse_ref(r)),
            (_, Some(r), _) => Position::Before(parse_ref(r)),
            (_, _, true) => Position::End,
            _ => {
                return Json(Wrote {
                    ok: false,
                    what_does_that_change: None,
                })
            }
        };
        let id = crate::cli::fresh_id(&l, &a.title);
        let ok =
            l.insert(Entry::Item(Item::new(id, a.title)), &pos).is_ok() && self.save(&l).is_ok();
        Json(Wrote {
            ok,
            what_does_that_change: None,
        })
    }

    #[tool(
        description = "Move Now forward — this is how work is completed. Completion \
                       is position, not state: an item is done when it sits behind \
                       Now. Record what actually happened in `result`."
    )]
    fn advance(&self, Parameters(a): Parameters<AdvanceArgs>) -> Json<Wrote> {
        let Ok((mut l, _)) = self.load() else {
            return Json(Wrote {
                ok: false,
                what_does_that_change: None,
            });
        };
        let target = a.to.as_deref().map(parse_ref);
        let Ok(passed) = l.advance(target.as_ref()) else {
            return Json(Wrote {
                ok: false,
                what_does_that_change: None,
            });
        };
        if let (Some(last), Some(text)) = (passed.last(), a.result.as_ref()) {
            if let Some(i) = l.index_of(&Ref::Id(last.clone())) {
                if let Entry::Item(item) = &mut l.entries[i] {
                    item.result = text.lines().map(str::to_string).collect();
                }
            }
        }
        let ok = self.save(&l).is_ok();
        Json(Wrote {
            ok,
            what_does_that_change: a.result.map(|_| INQUIRY.to_string()),
        })
    }

    #[tool(
        description = "Reorder an item. Changing the order is not a failure of \
                          planning — it is planning."
    )]
    fn move_item(&self, Parameters(a): Parameters<MoveArgs>) -> Json<Wrote> {
        let Ok((mut l, _)) = self.load() else {
            return Json(Wrote {
                ok: false,
                what_does_that_change: None,
            });
        };
        let pos = match (&a.after, &a.before) {
            (Some(r), _) => Position::After(parse_ref(r)),
            (_, Some(r)) => Position::Before(parse_ref(r)),
            _ => {
                return Json(Wrote {
                    ok: false,
                    what_does_that_change: None,
                })
            }
        };
        let ok = l.move_entry(&parse_ref(&a.id), &pos).is_ok() && self.save(&l).is_ok();
        Json(Wrote {
            ok,
            what_does_that_change: None,
        })
    }

    #[tool(
        description = "Lint the line against the practice: disguised buckets, \
                       unsharpened work inside the window, over-detailed work far \
                       from Now, hierarchy that belongs on the line."
    )]
    fn check(&self, Parameters(_): Parameters<Empty>) -> Json<CheckOut> {
        let Ok((l, cfg)) = self.load() else {
            return Json(CheckOut {
                findings: vec![],
                clean: false,
            });
        };
        let findings = check::check(&l, &cfg);
        Json(CheckOut {
            clean: findings.is_empty(),
            findings: findings
                .into_iter()
                .map(|f| FindingOut {
                    lint: f.lint.to_string(),
                    severity: match f.severity {
                        check::Severity::Warning => "warning".into(),
                        check::Severity::Error => "error".into(),
                    },
                    subject: f.subject,
                    message: f.message,
                })
                .collect(),
        })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct Empty {}

const INSTRUCTIONS: &str = "\
Throughline is a practice of continuous inquiry and forward flow. Intentions,
events, evidence and outcomes live in one ordered learning log running from
past, through Now, into the future.

Completion is POSITION, not state: an item is done when it sits behind Now. The
file format calls entries items, but the practice can use them for intentions,
events, evidence, outcomes, exclusions, and follow-up threads. Use `advance` to
move Now forward, and always record what actually happened in `result` — that is
where the practice keeps what it learned.

Then ask the question that moves things: what does that change? Act on the
answer with `move` or `add`. Placement is always required; there is no backlog.";

// Spelled out rather than using the macro's defaults. Those call
// Implementation::from_build_env(), whose env!("CARGO_CRATE_NAME") is
// evaluated inside rmcp — so the server introduces itself as "rmcp" — and
// they drop the instructions entirely, which are how an agent learns the
// vocabulary.
#[tool_handler]
impl ServerHandler for Throughline {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(rmcp::model::Implementation::new(
            "throughline",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(INSTRUCTIONS)
    }
}

pub async fn serve(path: PathBuf, root: PathBuf) -> anyhow::Result<()> {
    let service = Throughline::new(path, root)
        .serve(rmcp::transport::io::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::parse;

    fn fixture() -> (tempfile::TempDir, Throughline) {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
        let p = d.path().join(".throughline/line.md");
        let l = parse(
            "# T\n\n- [x] a  ^aaa\n      → shipped\n\n── NOW ──\n\n- [ ] b  ^bbb\n      why\n- [ ] c  ^ccc\n      why\n",
        )
        .unwrap();
        io::write_atomic(&p, &l).unwrap();
        let t = Throughline::new(p, d.path().to_path_buf());
        (d, t)
    }

    #[test]
    fn line_reports_history_and_intention() {
        let (_d, t) = fixture();
        let Json(out) = t.line();
        assert_eq!(out.entries.len(), 4);
        assert_eq!(out.entries[0].kind, "item");
        assert!(out.entries[0].behind_now);
        assert_eq!(out.entries[0].result, vec!["shipped"]);
        assert!(!out.entries[3].behind_now);
    }

    #[test]
    fn now_returns_the_next_item_ahead() {
        let (_d, t) = fixture();
        let Json(out) = t.now();
        assert_eq!(out.item.unwrap().id.unwrap(), "bbb");
    }

    #[test]
    fn add_without_placement_is_refused() {
        let (_d, t) = fixture();
        let Json(w) = t.add(Parameters(AddArgs {
            title: "x".into(),
            after: None,
            before: None,
            end: None,
        }));
        assert!(!w.ok, "placement is required");
    }

    #[test]
    fn add_after_a_ref_writes_to_disk() {
        let (_d, t) = fixture();
        let Json(w) = t.add(Parameters(AddArgs {
            title: "new".into(),
            after: Some("^bbb".into()),
            before: None,
            end: None,
        }));
        assert!(w.ok);
        // Re-read through a fresh call: the write must have hit the file.
        let Json(out) = t.line();
        let titles: Vec<_> = out.entries.iter().filter_map(|e| e.title.clone()).collect();
        assert_eq!(titles, ["a", "b", "new", "c"]);
    }

    #[test]
    fn advance_records_a_result_and_asks_what_it_changes() {
        let (_d, t) = fixture();
        let Json(w) = t.advance(Parameters(AdvanceArgs {
            to: None,
            result: Some("it worked".into()),
        }));
        assert!(w.ok);
        assert!(w
            .what_does_that_change
            .unwrap()
            .contains("what does that change"));

        let Json(out) = t.line();
        let b = out
            .entries
            .iter()
            .find(|e| e.id.as_deref() == Some("bbb"))
            .unwrap();
        assert!(b.behind_now, "advance must move it behind Now");
        assert_eq!(b.result, vec!["it worked"]);
    }

    #[test]
    fn advance_without_a_result_does_not_ask() {
        let (_d, t) = fixture();
        let Json(w) = t.advance(Parameters(AdvanceArgs {
            to: None,
            result: None,
        }));
        assert!(w.ok);
        assert!(w.what_does_that_change.is_none());
    }

    #[test]
    fn check_reports_a_clean_line() {
        let (_d, t) = fixture();
        let Json(c) = t.check(Parameters(Empty {}));
        assert!(c.clean, "unexpected findings: {:?}", c.findings.len());
    }

    #[test]
    fn tools_see_edits_made_outside_the_server() {
        let (_d, t) = fixture();
        // The file is the source of truth; a hand edit must be picked up.
        let l = parse("# T\n\n── NOW ──\n\n- [ ] edited by hand  ^zzz\n      body\n").unwrap();
        io::write_atomic(&t.path, &l).unwrap();
        let Json(out) = t.line();
        assert_eq!(out.entries[1].title.as_deref(), Some("edited by hand"));
    }
}

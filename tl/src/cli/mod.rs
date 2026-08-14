pub mod render;

use crate::config::Config;
use crate::format::io;
use crate::glyphs::{Glyphs, Mode};
use crate::check::{self, Severity};
use crate::model::{Child, Entry, Id, Item, ItemState, Line, Marker, Position, Ref};
use crate::theme::{Depth, Theme, Variant};
use crate::view;
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use render::Ctx;
use std::io::IsTerminal;
use std::path::Path;

#[derive(Parser)]
#[command(name = "tl", about = "Manage work as one ordered line")]
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
    /// Lint the line against the practice
    Check,
    /// Normalize derived content
    Fmt,
}

pub fn parse_ref(s: &str) -> Ref {
    match s {
        "now" | "NOW" => Ref::Now,
        other => match other.strip_prefix('^') {
            Some(id) => Ref::Id(Id::new(id)),
            None => Ref::Marker(other.to_string()),
        },
    }
}

fn placement(after: &Option<String>, before: &Option<String>, end: bool) -> Result<Position> {
    match (after, before, end) {
        (Some(a), _, _) => Ok(Position::After(parse_ref(a))),
        (_, Some(b), _) => Ok(Position::Before(parse_ref(b))),
        (_, _, true) => Ok(Position::End),
        _ => Err(anyhow!(
            "placement is required: pass --after, --before, or --end. \
             Choosing where work goes is the thinking the practice asks for."
        )),
    }
}

/// Ids are short, stable, and derived from content so two runs never collide.
pub(crate) fn fresh_id(line: &Line, seed: &str) -> Id {
    let mut hash: u64 = 1469598103934665603;
    for b in seed.bytes().chain(line.entries.len().to_le_bytes()) {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
    for attempt in 0..64u64 {
        let mut n = hash.wrapping_add(attempt);
        let mut sid = String::new();
        for _ in 0..3 {
            sid.push(alphabet[(n % 36) as usize] as char);
            n /= 36;
        }
        if line.item(&Id::new(sid.clone())).is_none() {
            return Id::new(sid);
        }
    }
    Id::new(format!("x{}", line.entries.len()))
}

/// Spec 4.4: a git SHA stops resolving the moment the commit is rebased or
/// amended, so a jj change ID is preferred wherever one is available.
/// `--commit auto` asks the repo; anything else is recorded verbatim.
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

/// Spec 3.4: the tool owes Inquiry a prompt, not a field.
fn inquiry_prompt(quiet: bool) {
    if !quiet {
        eprintln!("what does that change?  tl move · tl add · tl drop");
    }
}

fn load() -> Result<(Line, Config, std::path::PathBuf)> {
    let cwd = std::env::current_dir()?;
    let path = io::find_line_file(&cwd)
        .ok_or_else(|| anyhow!("no .throughline/line.md found — run `tl init` to start one"))?;
    let root = path
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(&cwd)
        .to_path_buf();
    let cfg = Config::load(&root);
    Ok((io::read(&path)?, cfg, path))
}

pub fn run(cli: Cli) -> Result<i32> {
    let (mut line, mut cfg, path) = load()?;

    let is_tty = std::io::stdout().is_terminal();
    let colour_on = match cli.color.as_str() {
        "always" => true,
        "never" => false,
        _ => is_tty,
    };
    // Write commands mutate and persist, then return. They run before `ctx`
    // borrows the line immutably.
    // Only --json silences the Inquiry prompt. It goes to stderr, so it never
    // pollutes stdout — and an agent shelling out is precisely who should be
    // asked what the result changes.
    let quiet = cli.json;
    let root = path.parent().and_then(|p| p.parent()).unwrap_or(Path::new(".")).to_path_buf();
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
            line.move_entry(&parse_ref(id), &placement(after, before, false)?)?;
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Advance { id, result, commit }) => {
            let rev = commit.as_deref().and_then(|c| resolve_commit(c, &root));
            let target = id.as_ref().map(|s| parse_ref(s));
            let passed = line.advance(target.as_ref())?;
            if let Some(last) = passed.last() {
                set_outcome(&mut line, last, result.clone(), rev);
            }
            io::write_atomic(&path, &line)?;
            if result.is_some() {
                inquiry_prompt(quiet);
            }
            return Ok(0);
        }
        Some(Command::Done { id, result, commit }) => {
            let rev = commit.as_deref().and_then(|c| resolve_commit(c, &root));
            let id = Id::new(id.trim_start_matches('^'));
            line.complete(&id)?;
            set_outcome(&mut line, &id, result.clone(), rev);
            io::write_atomic(&path, &line)?;
            if result.is_some() {
                inquiry_prompt(quiet);
            }
            return Ok(0);
        }
        Some(Command::Drop { id, why, result }) => {
            let id = Id::new(id.trim_start_matches('^'));
            line.drop_item(&id, why.clone())?;
            set_outcome(&mut line, &id, result.clone(), None);
            io::write_atomic(&path, &line)?;
            inquiry_prompt(quiet);
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
            let idx = line.index_of(&parse_ref(id)).ok_or_else(|| anyhow!("no item {id}"))?;
            if let Entry::Item(item) = &mut line.entries[idx] {
                item.description = body.lines().map(str::to_string).collect();
            }
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Split { id }) => {
            let idx = line.index_of(&parse_ref(id)).ok_or_else(|| anyhow!("no item {id}"))?;
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
            if let Some(b) = back {
                cfg.window_back = *b;
            }
            if let Some(a) = ahead {
                cfg.window_ahead = *a;
            }
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
        None => {
            // No subcommand: open the TUI.
            let mode = Mode::resolve(cli.glyphs.as_deref(), &cfg, is_tty);
            let variant = Variant::resolve(cli.theme.as_deref(), &cfg);
            crate::tui::launch(line, cfg, &path, mode, variant)?;
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

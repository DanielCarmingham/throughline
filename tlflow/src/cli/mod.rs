pub mod init;
pub mod render;

use crate::check::{self, Severity};
use crate::config::Config;
use crate::format::io;
use crate::glyphs::{Glyphs, Mode};
use crate::model::{Child, Entry, Id, Item, ItemState, Line, Marker, Position, Ref};
use crate::theme::{Depth, Theme, Variant};
use crate::view;
use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use render::Ctx;
use std::io::IsTerminal;
use std::path::Path;

const AFTER_HELP: &str = "\
REFS
  Commands that take a <ref> accept any of:
    ^k3f        an item id, as shown after each title
    v0.1        a marker label
    now         the Now boundary itself

THE SHAPE OF IT
  Work lives on one ordered line: history behind Now, intention ahead of it.
  Completion is POSITION, not state — an item is done when it sits behind Now.
  Placement is always required; there is no backlog to drop things into.

EXAMPLES
  tlflow                                   open the terminal UI
  tlflow window                            what is in focus right now
  tlflow add \"parse the file\" --after now  place work deliberately
  tlflow advance --result \"tests pass\"     complete, and record what happened
  tlflow move ^k3f --before v0.1           reorder; this is replanning
  tlflow check                             lint the line against the practice
  tlflow line --json                       machine-readable, for scripts and agents

  Full manual: docs/manual.md          The practice: https://tlflow.cc";

#[derive(Parser)]
#[command(
    name = "tlflow",
    about = "Manage work as one ordered line — a practice of continuous inquiry and forward flow",
    after_help = AFTER_HELP,
    version
)]
pub struct Cli {
    /// Glyph set: nerdfont, unicode or ascii. Piped output uses ascii unless
    /// this is given explicitly.
    #[arg(long, global = true, value_name = "MODE")]
    pub glyphs: Option<String>,
    /// Theme name: dark, light, or a file in .throughline/themes/.
    #[arg(long, global = true, value_name = "NAME")]
    pub theme: Option<String>,
    /// When to colour output: auto, always or never.
    #[arg(long, global = true, default_value = "auto", value_name = "WHEN")]
    pub color: String,
    /// Emit JSON instead of formatted text.
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
        /// Items of history to include (default 3).
        #[arg(long, value_name = "N")]
        back: Option<usize>,
        /// Items of intention to include (default 7).
        #[arg(long, value_name = "N")]
        ahead: Option<usize>,
    },
    /// A span, written `<ref>..<ref>`
    Slice {
        /// A span written `<ref>..<ref>`, e.g. `^k3f..now`.
        span: String,
    },
    /// Add an item. Placement is required.
    Add {
        /// What the work is.
        title: String,
        /// Place immediately after this ref.
        #[arg(long, group = "place", value_name = "REF")]
        after: Option<String>,
        /// Place immediately before this ref.
        #[arg(long, group = "place", value_name = "REF")]
        before: Option<String>,
        /// Place at the far end of the line.
        #[arg(long, group = "place")]
        end: bool,
    },
    /// Reorder — this is replanning
    Move {
        /// The item or marker to move.
        id: String,
        /// Move to just after this ref.
        #[arg(long, group = "place", value_name = "REF")]
        after: Option<String>,
        /// Move to just before this ref.
        #[arg(long, group = "place", value_name = "REF")]
        before: Option<String>,
    },
    /// Move Now forward — this is completion
    Advance {
        /// Advance past everything up to and including this ref. Omit for the
        /// next item.
        id: Option<String>,
        /// What actually happened. This is where the practice keeps what it
        /// learned.
        #[arg(long, value_name = "TEXT")]
        result: Option<String>,
        /// Link a revision. `auto` asks the repo, preferring a jj change ID
        /// because it survives rewriting where a git SHA does not.
        #[arg(long, value_name = "REV")]
        commit: Option<String>,
    },
    /// Complete an item out of order
    Done {
        /// The item to complete.
        id: String,
        /// What actually happened.
        #[arg(long, value_name = "TEXT")]
        result: Option<String>,
        /// Link a revision; `auto` asks the repo.
        #[arg(long, value_name = "REV")]
        commit: Option<String>,
    },
    /// The other outcome
    Drop {
        /// The item to drop.
        id: String,
        /// Why it will not be done.
        #[arg(long, value_name = "TEXT")]
        why: String,
        /// Anything learned in the process.
        #[arg(long, value_name = "TEXT")]
        result: Option<String>,
    },
    /// Place a landmark
    Mark {
        /// The landmark's name, e.g. `v0.1` or `launch`.
        label: String,
        /// Place immediately after this ref.
        #[arg(long, group = "place", value_name = "REF")]
        after: Option<String>,
        /// Place immediately before this ref.
        #[arg(long, group = "place", value_name = "REF")]
        before: Option<String>,
    },
    /// Change an item's title. The line is hand-editable, but a verb keeps
    /// scripts and agents from having to rewrite Markdown.
    Retitle {
        /// The item or marker to rename.
        id: String,
        /// The new title.
        title: String,
    },
    /// Add or replace an item's body
    Sharpen {
        /// The item to sharpen.
        id: String,
        /// The body text. Having one is what "sharpened" means.
        #[arg(long, value_name = "TEXT")]
        body: String,
    },
    /// Promote children onto the line
    Split {
        /// The parent whose children become items on the line.
        id: String,
    },
    /// Lint the line against the practice
    Check,
    /// Normalize derived content
    Fmt,
    /// Create .throughline/, a practice summary, and the agent stanza
    Init,
    /// Re-run glyph and theme capability detection
    Doctor,
    /// Seed a line from a plan document
    Plan {
        /// A markdown plan; `### Task N: Title` headings become items.
        file: std::path::PathBuf,
    },
    /// Serve the line to agents as MCP tools over stdio
    Mcp,
    /// Print a generated method-document diagram
    Diagram {
        /// Which diagram. Omit to list the available names.
        name: Option<String>,
        /// Print every diagram, fenced and ready to paste.
        #[arg(long)]
        all: bool,
    },
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
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    };
    if root.join(".jj").is_dir() {
        // Change IDs survive rewriting; commit IDs do not.
        if let Some(id) = run(
            "jj",
            &["log", "-r", "@", "--no-graph", "-T", "change_id.short()"],
        ) {
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
        eprintln!("what does that change?  tlflow move · tlflow add · tlflow drop");
    }
}

/// A named theme may be built in or a user file. Anything explicitly asked for
/// must exist — silently falling back to dark would hide a typo.
fn resolve_theme(flag: Option<&str>, cfg: &Config, depth: Depth, root: &Path) -> Result<Theme> {
    let named = flag
        .map(str::to_string)
        .or_else(|| std::env::var("TL_THEME").ok())
        .or_else(|| cfg.theme.clone());
    match named {
        Some(name) => Theme::load(&name, depth, root).map_err(|e| anyhow!(e)),
        None => {
            // Only ask the terminal for its background when colour is actually
            // going to be used. OSC 11 writes an escape sequence and waits for
            // a reply; doing that on every piped run is a wasted round trip
            // and leaves visible junk in terminals that never answer.
            let variant = if depth == Depth::None {
                Variant::Dark
            } else {
                Variant::detect()
            };
            Ok(Theme::new(variant, depth))
        }
    }
}

fn load() -> Result<(Line, Config, std::path::PathBuf)> {
    let cwd = std::env::current_dir()?;
    let path = io::find_line_file(&cwd)
        .ok_or_else(|| anyhow!("no .throughline/line.md found — run `tlflow init` to start one"))?;
    let root = path
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(&cwd)
        .to_path_buf();
    let cfg = Config::load(&root);
    Ok((io::read(&path)?, cfg, path))
}

pub fn run(cli: Cli) -> Result<i32> {
    // These two run before load(): `init` exists precisely because there is no
    // line yet, and `doctor` only needs the glyph tables.
    if let Some(Command::Init) = cli.command {
        let root = std::env::current_dir()?;
        init::scaffold(&root)?;
        println!("initialised .throughline/ — run `tlflow` to open the line");
        return Ok(0);
    }
    if let Some(Command::Doctor) = cli.command {
        print!("{}", init::sample_rows());
        let root = std::env::current_dir()?;
        println!("\nthemes: {}", Theme::available(&root).join(", "));
        return Ok(0);
    }
    if let Some(Command::Diagram { name, all }) = &cli.command {
        if *all {
            print!("{}", crate::diagrams::render_all());
        } else if let Some(n) = name {
            match crate::diagrams::render(n) {
                Some(d) => print!("{d}"),
                None => return Err(anyhow!("no diagram named {n}")),
            }
        } else {
            println!("{}", crate::diagrams::NAMES.join("\n"));
        }
        return Ok(0);
    }

    if let Some(Command::Mcp) = cli.command {
        // Load once to fail fast on a missing or malformed line, then hand the
        // paths to the server, which re-reads on every tool call.
        let (_, _, path) = load()?;
        let root = path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(crate::mcp::serve(path, root))?;
        return Ok(0);
    }

    let (mut line, mut cfg, path) = load()?;

    let is_tty = std::io::stdout().is_terminal();
    let force_colour = match cli.color.as_str() {
        "always" => Some(true),
        "never" => Some(false),
        _ => None,
    };
    // Write commands mutate and persist, then return. They run before `ctx`
    // borrows the line immutably.
    // Only --json silences the Inquiry prompt. It goes to stderr, so it never
    // pollutes stdout — and an agent shelling out is precisely who should be
    // asked what the result changes.
    let quiet = cli.json;
    let root = path
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(Path::new("."))
        .to_path_buf();
    match &cli.command {
        Some(Command::Add {
            title,
            after,
            before,
            end,
        }) => {
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
        Some(Command::Mark {
            label,
            after,
            before,
        }) => {
            line.insert(
                Entry::Marker(Marker {
                    label: label.clone(),
                }),
                &placement(after, before, false)?,
            )?;
            io::write_atomic(&path, &line)?;
            return Ok(0);
        }
        Some(Command::Retitle { id, title }) => {
            let idx = line
                .index_of(&parse_ref(id))
                .ok_or_else(|| anyhow!("no item {id}"))?;
            match &mut line.entries[idx] {
                Entry::Item(item) => item.title = title.clone(),
                Entry::Marker(m) => m.label = title.clone(),
                Entry::Now => return Err(anyhow!("NOW has no title")),
            }
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
        Some(Command::Plan { file }) => {
            let added = init::from_plan(file, &mut line)?;
            io::write_atomic(&path, &line)?;
            println!("seeded {added} items from {}", file.display());
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

    let depth = Depth::resolve(force_colour, is_tty);
    // Resolved once. The TUI always draws in truecolour, but must not trigger
    // a second terminal query, so it clones the same resolution.
    let theme = resolve_theme(cli.theme.as_deref(), &cfg, depth, &root)?;
    let tui_theme = theme.with_depth(Depth::True);
    let ctx = Ctx {
        glyphs: Glyphs::for_mode(Mode::resolve(cli.glyphs.as_deref(), &cfg, is_tty)),
        theme,
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
            // No subcommand: open the TUI. Without a terminal, crossterm fails
            // with a bare errno, so say what to do instead.
            if !is_tty || !std::io::stdin().is_terminal() {
                return Err(anyhow!(
                    "the TUI needs a terminal. For non-interactive use try \
                     `tlflow line`, `tlflow window` or `tlflow now` — add --json for a \
                     machine-readable form."
                ));
            }
            let mode = Mode::resolve(cli.glyphs.as_deref(), &cfg, is_tty);
            // Reuse the theme already resolved above: resolving again would
            // send a second OSC 11 query for no reason.
            crate::tui::launch(line, cfg, &path, mode, tui_theme)?;
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

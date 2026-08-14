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

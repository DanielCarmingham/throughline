pub mod app;
pub mod list;
pub mod ribbon;

use crate::config::Config;
use crate::glyphs::{Glyphs, Mode};
// `ratatui::prelude::*` also exports `Line`, so alias ours to keep them apart.
use crate::model::Line as ProjectLine;
use crate::theme::{Theme, Token};
#[cfg(test)]
use crate::theme::{Depth, Variant};
use crate::view;
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::path::Path;

const HELP: &str = "j/k move · J/K reorder · n Now · space advance · a add · s sharpen \
                    · m mark · d drop · [/] window · / search · t theme · ? help · q quit";

/// Restore the terminal before anything is printed.
///
/// A panic inside the alternate screen writes its message to that screen,
/// which the terminal then discards on restore — so the program appears to
/// flash and vanish with no explanation. This hook tears the screen down
/// first, so whatever went wrong is readable on the normal screen.
fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
        eprintln!("tlflow: the terminal UI crashed. This is a bug.");
        original(info);
    }));
}

pub fn launch(
    line: ProjectLine,
    cfg: Config,
    path: &Path,
    mode: Mode,
    theme: Theme,
) -> Result<()> {
    install_panic_hook();
    enable_raw_mode()?;
    let mut out = std::io::stdout();
    crossterm::execute!(out, EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;

    let mut app = app::App::new(line, cfg);
    app.theme = theme;
    let glyphs = Glyphs::for_mode(mode);

    let result = run_loop(&mut term, &mut app, &glyphs, path);

    // Restore unconditionally, before returning any error, so the message is
    // printed to a working terminal rather than to a screen about to vanish.
    disable_raw_mode()?;
    crossterm::execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;
    result.context("while running the terminal UI")
}

fn run_loop<B: Backend>(
    term: &mut Terminal<B>,
    app: &mut app::App,
    glyphs: &Glyphs,
    path: &Path,
) -> Result<()> {
    loop {
        let theme = app.theme.clone();
        term.draw(|f| draw(f, app, glyphs, &theme))?;

        let ev = event::read().context("reading a terminal event")?;
        if let Event::Key(k) = ev {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            // While a prompt is open, keys feed the buffer instead of the keymap.
            if app.prompt.is_some() {
                match k.code {
                    KeyCode::Enter => app.commit_prompt(),
                    KeyCode::Esc => {
                        app.prompt = None;
                        app.buffer.clear();
                    }
                    KeyCode::Backspace => {
                        app.buffer.pop();
                    }
                    KeyCode::Char(c) => app.buffer.push(c),
                    _ => {}
                }
                continue;
            }
            if app::Action::Quit == app.on_key(k.code) {
                break;
            }
        }

        if app.dirty {
            crate::format::io::write_atomic(path, &app.line)
                .with_context(|| format!("saving {}", path.display()))?;
            app.dirty = false;
        }
    }
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
    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(f.area());

    // Zoom out: the whole project as one row.
    let window = view::window(&app.line, &app.cfg);
    let segs = ribbon::build(&app.line, window, glyphs, chunks[0].width as usize);
    f.render_widget(Paragraph::new(to_line(&segs, theme)), chunks[0]);

    // Zoom in: readable titles for what is in the window.
    let rows: Vec<Line> = list::build(app, glyphs)
        .iter()
        .map(|r| to_line(r, theme))
        .collect();
    f.render_widget(Paragraph::new(rows), chunks[1]);

    let status = if let Some(p) = app.prompt {
        let label = match p {
            app::Prompt::Add => "add",
            app::Prompt::Sharpen => "sharpen",
            app::Prompt::Mark => "mark",
            app::Prompt::Search => "search",
        };
        Line::from(vec![Span::styled(
            format!("{label}: {}", app.buffer),
            theme.style(Token::Cursor),
        )])
    } else if app.help {
        Line::from(vec![Span::styled(HELP, theme.style(Token::Muted))])
    } else {
        Line::from(vec![Span::styled(
            "? help · q quit",
            theme.style(Token::Muted),
        )])
    };
    f.render_widget(Paragraph::new(status), chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::parse;
    use ratatui::backend::TestBackend;

    fn sample() -> ProjectLine {
        parse("# T\n\n- [x] a  ^aaa\n\n── NOW ──\n\n- [ ] b  ^bbb\n      why b matters\n- [ ] c  ^ccc\n")
            .unwrap()
    }

    /// Drawing must not panic at any plausible terminal size — including ones
    /// too small to hold the ribbon.
    #[test]
    fn draws_at_a_range_of_terminal_sizes() {
        for (w, h) in [(80, 24), (40, 10), (200, 60), (20, 5), (10, 3)] {
            let backend = TestBackend::new(w, h);
            let mut term = Terminal::new(backend).unwrap();
            let app = app::App::new(sample(), Config::default());
            let theme = Theme::new(Variant::Dark, Depth::True);
            let glyphs = Glyphs::for_mode(Mode::Ascii);
            term.draw(|f| draw(f, &app, &glyphs, &theme))
                .unwrap_or_else(|e| panic!("draw failed at {w}x{h}: {e}"));
        }
    }

    #[test]
    fn the_rendered_screen_shows_both_zoom_levels() {
        let backend = TestBackend::new(60, 12);
        let mut term = Terminal::new(backend).unwrap();
        let app = app::App::new(sample(), Config::default());
        let theme = Theme::new(Variant::Dark, Depth::None);
        let glyphs = Glyphs::for_mode(Mode::Ascii);
        term.draw(|f| draw(f, &app, &glyphs, &theme)).unwrap();

        let text: String = term.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("[x]"), "ribbon missing from: {text}");
        assert!(text.contains("^bbb"), "window list missing titles");
        assert!(text.contains("q quit"), "status bar missing");
    }
}

#[cfg(test)]
mod real_line_tests {
    use super::*;
    use ratatui::backend::TestBackend;

    /// Draw the project's ACTUAL line — 20+ items with long results — at many
    /// terminal sizes. The other draw test uses a four-entry fixture, which is
    /// not representative of anything real.
    #[test]
    fn draws_the_real_project_line_at_many_sizes() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../.throughline/line.md");
        let Ok(line) = crate::format::io::read(std::path::Path::new(path)) else {
            eprintln!("no project line to test against; skipping");
            return;
        };
        for (w, h) in [(80, 24), (120, 40), (60, 20), (40, 12), (30, 8), (20, 6), (12, 4)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            let app = app::App::new(line.clone(), Config::default());
            let theme = Theme::new(crate::theme::Variant::Dark, crate::theme::Depth::True);
            let glyphs = Glyphs::for_mode(Mode::Unicode);
            term.draw(|f| draw(f, &app, &glyphs, &theme))
                .unwrap_or_else(|e| panic!("draw failed at {w}x{h}: {e}"));
        }
    }
}

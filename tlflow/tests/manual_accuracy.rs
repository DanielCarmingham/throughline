//! Check the manual against the binary.
//!
//! Documentation that drifts is worse than none: it teaches commands that do
//! not exist. These tests fail when the two disagree.

use assert_cmd::Command;

fn manual() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/manual.md"))
        .expect("docs/manual.md must exist")
}

fn help() -> String {
    let out = Command::cargo_bin("tlflow").unwrap().arg("--help").output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

/// Every subcommand the binary has must appear in the manual.
#[test]
fn the_manual_covers_every_command() {
    let m = manual();
    let h = help();
    // Parse the command list out of --help.
    let mut names = Vec::new();
    let mut in_commands = false;
    for line in h.lines() {
        if line.starts_with("Commands:") {
            in_commands = true;
            continue;
        }
        if in_commands {
            if line.trim().is_empty() {
                break;
            }
            if let Some(first) = line.split_whitespace().next() {
                names.push(first.to_string());
            }
        }
    }
    assert!(names.len() > 15, "failed to parse commands from --help");

    let missing: Vec<&String> = names
        .iter()
        .filter(|n| n.as_str() != "help")
        .filter(|n| !m.contains(&format!("tlflow {n}")))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/manual.md does not mention: {missing:?}"
    );
}

/// Every keymap entry in the manual must exist in the TUI's own help string.
#[test]
fn the_documented_keymap_matches_the_tui() {
    let m = manual();
    for key in ["`n`", "`space`", "`a`", "`s`", "`m`", "`d`", "`/`", "`t`", "`?`", "`q`"] {
        assert!(m.contains(key), "manual is missing key {key}");
    }
    // The TUI's own status-bar help must agree on the same verbs.
    let tui_help = include_str!("../src/tui/mod.rs");
    for verb in ["advance", "add", "sharpen", "mark", "drop", "search", "help", "quit"] {
        assert!(
            tui_help.contains(verb),
            "the TUI help string is missing {verb}"
        );
    }
}

/// Every lint named in the manual must be a real lint.
#[test]
fn the_documented_lints_all_exist() {
    let m = manual();
    let source = include_str!("../src/check/mod.rs");
    for lint in [
        "bucket",
        "unsharpened",
        "false-certainty",
        "result-ahead",
        "orphan-parent",
        "independent-children",
        "duplicate-id",
        "no-now",
    ] {
        assert!(m.contains(lint), "manual is missing lint {lint}");
        assert!(
            source.contains(&format!("\"{lint}\"")),
            "lint {lint} is documented but does not exist"
        );
    }
}

/// Every theme token named in the manual must be a real token.
#[test]
fn the_documented_theme_tokens_all_exist() {
    let m = manual();
    let source = include_str!("../src/theme/mod.rs");
    for tok in [
        "past", "now", "near", "mid", "far", "marker", "blocked", "dropped", "cursor",
        "window", "muted", "bg", "fg",
    ] {
        assert!(m.contains(tok), "manual is missing token {tok}");
        assert!(
            source.contains(&format!("(\"{tok}\", Token::")),
            "token {tok} is documented but not in TOKEN_NAMES"
        );
    }
}

/// The config keys the manual shows must be the ones Config actually reads.
#[test]
fn the_documented_config_keys_are_real() {
    let m = manual();
    let source = include_str!("../src/config.rs");
    for key in ["window_back", "window_ahead", "far_body_lines", "glyphs", "theme"] {
        assert!(m.contains(key), "manual is missing config key {key}");
        assert!(
            source.contains(key),
            "config key {key} is documented but not read"
        );
    }
    assert!(m.contains("allow_markers"));
}

/// --help points at the manual; the manual must therefore be findable.
#[test]
fn help_points_somewhere_real() {
    let h = help();
    assert!(h.contains("docs/manual.md"), "--help should name the manual");
    assert!(h.contains("tlflow.cc"), "--help should name the site");
    assert!(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/manual.md")).is_file()
    );
}

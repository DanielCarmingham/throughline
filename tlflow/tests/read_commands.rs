use assert_cmd::Command;
use std::path::Path;

const LINE: &str = "\
# Throughline — Fixture

## Line

- [x] sketch the practice  ^k3f
      → ten properties

── NOW ──

- [ ] write the docs  ^q1d
      the full practice
- [ ] parse line.md  ^r7e
      grammar and errors

◆ v0.1 ◆

- [ ] build the tui  ^t9a
      ribbon and list
";

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".throughline")).unwrap();
    std::fs::write(dir.path().join(".throughline/line.md"), LINE).unwrap();
    dir
}

fn tl(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("tlflow").unwrap();
    c.current_dir(dir);
    c
}

#[test]
fn line_prints_every_entry_in_order() {
    let d = fixture();
    let out = tl(d.path()).arg("line").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let order = [
        "sketch the practice",
        "write the docs",
        "parse line.md",
        "v0.1",
        "build the tui",
    ];
    let mut last = 0;
    for needle in order {
        let at = text.find(needle).unwrap_or_else(|| panic!("missing {needle}"));
        assert!(at >= last, "{needle} out of order");
        last = at;
    }
}

#[test]
fn non_tty_output_is_ascii_with_no_escape_codes() {
    let d = fixture();
    let out = tl(d.path()).arg("line").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        !text.contains('\u{1b}'),
        "escape codes leaked into piped output"
    );
    assert!(text.is_ascii(), "non-ascii glyphs leaked into piped output");
}

#[test]
fn now_reports_the_next_item_ahead() {
    let d = fixture();
    tl(d.path())
        .arg("now")
        .assert()
        .success()
        .stdout(predicates::str::contains("write the docs"));
}

#[test]
fn window_is_narrower_than_the_whole_line() {
    let d = fixture();
    let out = tl(d.path())
        .args(["window", "--ahead", "1"])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.contains("write the docs"));
    assert!(!text.contains("build the tui"));
}

#[test]
fn slice_returns_only_the_requested_span() {
    let d = fixture();
    let out = tl(d.path()).args(["slice", "^q1d..^r7e"]).assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.contains("write the docs"));
    assert!(text.contains("parse line.md"));
    assert!(!text.contains("sketch the practice"));
}

#[test]
fn json_output_carries_the_stable_field_names() {
    let d = fixture();
    let out = tl(d.path()).args(["line", "--json"]).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let entries = v["entries"].as_array().unwrap();
    assert_eq!(entries[0]["kind"], "item");
    assert_eq!(entries[0]["id"], "k3f");
    assert_eq!(entries[0]["title"], "sketch the practice");
    assert_eq!(entries[0]["behind_now"], true);
    assert_eq!(entries[1]["kind"], "now");
    assert_eq!(v["now_index"], 1);
}

#[test]
fn json_marks_future_items_as_ahead() {
    let d = fixture();
    let out = tl(d.path()).args(["now", "--json"]).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["item"]["behind_now"], false);
}

#[test]
fn a_missing_line_file_fails_with_a_useful_message() {
    let empty = tempfile::tempdir().unwrap();
    tl(empty.path())
        .arg("line")
        .assert()
        .failure()
        .stderr(predicates::str::contains("tlflow init"));
}

#[test]
fn a_malformed_line_file_reports_the_line_number() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# T\n\n── NOW ──\n\n- [ ] no id here\n",
    )
    .unwrap();
    tl(d.path())
        .arg("line")
        .assert()
        .failure()
        .stderr(predicates::str::contains("line.md:5"));
}

#[test]
fn launching_the_tui_without_a_terminal_says_what_to_do_instead() {
    // Piped or in CI, crossterm fails with a bare errno. The message should
    // point at the non-interactive commands rather than an OS error number.
    let d = fixture();
    tl(d.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("needs a terminal"))
        .stderr(predicates::str::contains("tlflow line"));
}

#[test]
fn color_always_emits_escape_codes_even_when_piped() {
    let d = fixture();
    let out = tl(d.path())
        .args(["line", "--color", "always"])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        text.contains('\u{1b}'),
        "--color always produced no escape codes when piped"
    );
}

#[test]
fn an_unknown_theme_name_fails_with_guidance() {
    let d = fixture();
    tl(d.path())
        .args(["line", "--theme", "nope"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no theme named nope"))
        .stderr(predicates::str::contains(".throughline/themes"));
}

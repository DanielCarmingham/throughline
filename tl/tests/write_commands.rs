use assert_cmd::Command;
use std::path::{Path, PathBuf};

const LINE: &str = "\
# Fixture

## Line

- [x] sketch  ^k3f

── NOW ──

- [ ] docs  ^q1d
      the full practice
- [ ] parse  ^r7e
      grammar

◆ v0.1 ◆

- [ ] tui  ^t9a
      ribbon
";

fn line_path(dir: &Path) -> PathBuf {
    dir.join(".throughline/line.md")
}

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".throughline")).unwrap();
    std::fs::write(line_path(dir.path()), LINE).unwrap();
    dir
}

fn tl(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("tl").unwrap();
    c.current_dir(dir);
    c
}

fn read(dir: &Path) -> String {
    std::fs::read_to_string(line_path(dir)).unwrap()
}

#[test]
fn add_requires_a_placement() {
    let d = fixture();
    tl(d.path()).args(["add", "new work"]).assert().failure();
}

#[test]
fn add_after_places_the_item_immediately_after() {
    let d = fixture();
    tl(d.path())
        .args(["add", "new work", "--after", "^q1d"])
        .assert()
        .success();
    let text = read(d.path());
    let a = text.find("docs").unwrap();
    let b = text.find("new work").unwrap();
    let c = text.find("parse").unwrap();
    assert!(a < b && b < c);
}

#[test]
fn add_after_a_marker_expresses_post_launch_work_as_a_position() {
    let d = fixture();
    tl(d.path())
        .args(["add", "cleanup", "--after", "v0.1"])
        .assert()
        .success();
    let text = read(d.path());
    assert!(text.find("◆ v0.1 ◆").unwrap() < text.find("cleanup").unwrap());
}

#[test]
fn advance_moves_now_and_rewrites_the_checkbox() {
    let d = fixture();
    tl(d.path()).arg("advance").assert().success();
    let text = read(d.path());
    assert!(text.contains("- [x] docs  ^q1d"));
    assert!(text.find("docs").unwrap() < text.find("── NOW ──").unwrap());
}

#[test]
fn advance_records_a_result_and_asks_what_it_changes() {
    let d = fixture();
    tl(d.path())
        .args(["advance", "--result", "shipped it"])
        .assert()
        .success()
        .stderr(predicates::str::contains("what does that change"));
    assert!(read(d.path()).contains("→ shipped it"));
}

#[test]
fn done_completes_an_item_out_of_order() {
    let d = fixture();
    tl(d.path()).args(["done", "^t9a"]).assert().success();
    let text = read(d.path());
    assert!(text.contains("- [x] tui  ^t9a"));
    assert!(text.find("tui").unwrap() < text.find("── NOW ──").unwrap());
}

#[test]
fn drop_records_the_reason_and_moves_the_item_behind_now() {
    let d = fixture();
    tl(d.path())
        .args(["drop", "^r7e", "--why", "superseded"])
        .assert()
        .success();
    assert!(read(d.path()).contains("- [-] parse  ^r7e  @dropped(superseded)"));
}

#[test]
fn move_reorders_without_duplicating() {
    let d = fixture();
    tl(d.path())
        .args(["move", "^t9a", "--before", "^q1d"])
        .assert()
        .success();
    let text = read(d.path());
    assert_eq!(text.matches("^t9a").count(), 1);
    assert!(text.find("tui").unwrap() < text.find("docs").unwrap());
}

#[test]
fn mark_places_a_landmark() {
    let d = fixture();
    tl(d.path())
        .args(["mark", "v0.2", "--after", "^t9a"])
        .assert()
        .success();
    assert!(read(d.path()).contains("◆ v0.2 ◆"));
}

#[test]
fn sharpen_adds_a_body() {
    let d = fixture();
    tl(d.path())
        .args(["sharpen", "^t9a", "--body", "ribbon plus window list"])
        .assert()
        .success();
    assert!(read(d.path()).contains("      ribbon plus window list"));
}

#[test]
fn split_promotes_children_onto_the_line_in_order() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        line_path(d.path()),
        "# T\n\n── NOW ──\n\n- [ ] recovery  ^rrr\n      - [ ] token\n      - [ ] email\n- [ ] after  ^zzz\n",
    )
    .unwrap();

    tl(d.path()).args(["split", "^rrr"]).assert().success();
    let text = read(d.path());

    assert!(text.find("recovery").unwrap() < text.find("token").unwrap());
    assert!(text.find("token").unwrap() < text.find("email").unwrap());
    assert!(text.find("email").unwrap() < text.find("after").unwrap());
    assert!(!text.contains("      - [ ] token"));
}

#[test]
fn fmt_normalizes_ascii_syntax_and_wrong_checkboxes() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        line_path(d.path()),
        "# T\n\n- [ ] past  ^aaa\n\n-- NOW --\n\n<> v1 <>\n\n- [x] future  ^bbb\n",
    )
    .unwrap();
    tl(d.path()).arg("fmt").assert().success();
    let text = read(d.path());
    assert!(text.contains("── NOW ──"));
    assert!(text.contains("◆ v1 ◆"));
    assert!(text.contains("- [x] past  ^aaa"));
    assert!(text.contains("- [ ] future  ^bbb"));
}

#[test]
fn an_explicit_commit_value_is_recorded_verbatim() {
    let d = fixture();
    tl(d.path())
        .args(["advance", "--commit", "88ca65b"])
        .assert()
        .success();
    assert!(read(d.path()).contains("@commit(88ca65b)"));
}

#[test]
fn commit_auto_never_records_the_literal_word() {
    let d = fixture();
    std::fs::create_dir_all(d.path().join(".jj")).unwrap();
    tl(d.path())
        .args(["advance", "--commit", "auto"])
        .assert()
        .success();
    assert!(
        !read(d.path()).contains("@commit(auto)"),
        "the literal 'auto' was recorded"
    );
}

#[test]
fn check_exits_zero_when_only_warnings_fire() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(line_path(d.path()), "# T\n\n── NOW ──\n\n- [ ] bare  ^aaa\n").unwrap();
    tl(d.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicates::str::contains("unsharpened"));
}

#[test]
fn check_exits_non_zero_on_an_error_lint() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(
        line_path(d.path()),
        "# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n      body\n      → premature\n",
    )
    .unwrap();
    tl(d.path())
        .arg("check")
        .assert()
        .failure()
        .stdout(predicates::str::contains("result-ahead"));
}

#[test]
fn check_json_lists_findings_with_severities() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".throughline")).unwrap();
    std::fs::write(line_path(d.path()), "# T\n\n── NOW ──\n\n- [ ] bare  ^aaa\n").unwrap();
    let out = tl(d.path()).args(["check", "--json"]).assert().success();
    let v: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(v["findings"][0]["lint"], "unsharpened");
    assert_eq!(v["findings"][0]["severity"], "warning");
}

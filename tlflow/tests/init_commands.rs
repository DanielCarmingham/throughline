use assert_cmd::Command;
use std::path::Path;

fn tl(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("tlflow").unwrap();
    c.current_dir(dir);
    c
}

#[test]
fn init_creates_a_parseable_line_and_the_agent_stanza() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();

    assert!(d.path().join(".throughline/line.md").is_file());
    assert!(d.path().join("THROUGHLINE.md").is_file());
    assert!(d.path().join("AGENTS.md").is_file());

    // The freshly created line must be readable by the tool itself.
    tl(d.path()).arg("line").assert().success();
}

#[test]
fn init_is_idempotent_and_never_clobbers_an_existing_line() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    std::fs::write(
        d.path().join(".throughline/line.md"),
        "# Mine\n\n── NOW ──\n\n- [ ] keep me  ^kkk\n",
    )
    .unwrap();
    tl(d.path()).arg("init").assert().success();

    let text = std::fs::read_to_string(d.path().join(".throughline/line.md")).unwrap();
    assert!(text.contains("keep me"));
}

#[test]
fn the_agent_stanza_names_the_commands_and_the_discipline() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    let text = std::fs::read_to_string(d.path().join("AGENTS.md")).unwrap();
    for needle in [
        "tlflow window",
        "tlflow add",
        "tlflow advance",
        "tlflow check",
        "behind Now",
        "what does that change",
    ] {
        assert!(text.contains(needle), "AGENTS.md is missing {needle}");
    }
}

#[test]
fn the_practice_summary_uses_the_current_vocabulary() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    let text = std::fs::read_to_string(d.path().join("THROUGHLINE.md")).unwrap();
    assert!(text.contains("Learn in cycles. Move in a line."));
    assert!(text.contains("Inquiry"));
    // The old framing must not come back.
    assert!(!text.contains("project management"));
}

#[test]
fn doctor_prints_all_three_glyph_modes_for_comparison() {
    let d = tempfile::tempdir().unwrap();
    let out = tl(d.path()).arg("doctor").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.contains("nerdfont"));
    assert!(text.contains("unicode"));
    assert!(text.contains("ascii"));
}

#[test]
fn doctor_works_without_a_line_file() {
    // It reports terminal capability, which has nothing to do with a line.
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("doctor").assert().success();
}

#[test]
fn plan_seeds_a_line_from_a_markdown_document() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    std::fs::write(
        d.path().join("plan.md"),
        "# A Plan\n\n### Task 1: First thing\n\nbody\n\n### Task 2: Second thing\n\nbody\n",
    )
    .unwrap();

    tl(d.path()).args(["plan", "plan.md"]).assert().success();
    let text = std::fs::read_to_string(d.path().join(".throughline/line.md")).unwrap();
    assert!(text.contains("First thing"));
    assert!(text.contains("Second thing"));
    assert!(text.find("First thing").unwrap() < text.find("Second thing").unwrap());
}

#[test]
fn plan_places_seeded_work_ahead_of_now() {
    let d = tempfile::tempdir().unwrap();
    tl(d.path()).arg("init").assert().success();
    std::fs::write(d.path().join("p.md"), "### Task 1: Only thing\n").unwrap();
    tl(d.path()).args(["plan", "p.md"]).assert().success();

    let text = std::fs::read_to_string(d.path().join(".throughline/line.md")).unwrap();
    assert!(text.find("── NOW ──").unwrap() < text.find("Only thing").unwrap());
}

use assert_cmd::Command;

#[test]
fn the_cli_can_print_a_diagram_by_name() {
    Command::cargo_bin("tlflow")
        .unwrap()
        .args(["diagram", "the-line"])
        .assert()
        .success();
}

#[test]
fn an_unknown_diagram_name_fails() {
    Command::cargo_bin("tlflow")
        .unwrap()
        .args(["diagram", "nope"])
        .assert()
        .failure();
}

/// Spec 9.3: the document cannot drift from the tool's real output, because the
/// output IS the document. If this fails, run `tlflow diagram --all` and paste.
#[test]
fn every_generated_diagram_appears_verbatim_in_the_method_document() {
    let doc = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/method.md"))
        .expect("docs/method.md must exist");

    for name in throughline::diagrams::NAMES {
        let rendered = throughline::diagrams::render(name).unwrap();
        assert!(
            doc.contains(rendered.trim_end()),
            "docs/method.md has drifted from `tlflow diagram {name}`.\n\
             Regenerate with: tlflow diagram --all\n\n\
             expected to find:\n{rendered}"
        );
    }
}

/// The document must not quietly revert to the old framing.
#[test]
fn the_method_document_uses_the_current_vocabulary() {
    let doc =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/method.md")).unwrap();
    for needle in [
        "Learn in cycles. Move in a line.",
        "Inquiry",
        "the Window",
        "Status is position",
    ] {
        assert!(doc.contains(needle), "method.md is missing {needle:?}");
    }
    assert!(
        !doc.contains("Throughline Method"),
        "method.md still calls it a method"
    );
}

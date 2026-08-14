use crate::config::Config;
use crate::model::{Entry, ItemState, Line};
use crate::view;

const BUCKET_WORDS: [&str; 7] = [
    "backlog",
    "someday",
    "later",
    "v2",
    "post-launch",
    "icebox",
    "blocked",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub lint: &'static str,
    pub severity: Severity,
    pub subject: String,
    pub message: String,
}

pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

pub fn check(line: &Line, cfg: &Config) -> Vec<Finding> {
    let mut out = Vec::new();
    let win = view::window(line, cfg);
    let now = line.now_index();

    for (i, entry) in line.entries.iter().enumerate() {
        match entry {
            Entry::Marker(m) => {
                let label = m.label.to_lowercase();
                let is_bucket = BUCKET_WORDS.iter().any(|w| label.contains(w));
                let allowed = cfg
                    .allow_markers()
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(&m.label));
                if is_bucket && !allowed {
                    out.push(Finding {
                        lint: "bucket",
                        severity: Severity::Warning,
                        subject: m.label.clone(),
                        message: "reads as a bucket; if it should happen later, put it later"
                            .into(),
                    });
                }
            }
            Entry::Item(item) => {
                let behind = i < now;
                let in_window = i >= win.start && i < win.end;

                if !behind && !item.result.is_empty() {
                    out.push(Finding {
                        lint: "result-ahead",
                        severity: Severity::Error,
                        subject: item.id.0.clone(),
                        message: "outcomes belong to history; move it behind Now first".into(),
                    });
                }

                if !behind && in_window && !item.is_sharpened() {
                    out.push(Finding {
                        lint: "unsharpened",
                        severity: Severity::Warning,
                        subject: item.id.0.clone(),
                        message: "inside the window but still bare; sharpen before starting".into(),
                    });
                }

                if !behind && !in_window && item.description.len() > cfg.far_body_lines {
                    out.push(Finding {
                        lint: "false-certainty",
                        severity: Severity::Warning,
                        subject: item.id.0.clone(),
                        message: "detailed but far from Now; distance should mean less detail"
                            .into(),
                    });
                }

                if behind
                    && !matches!(item.state, ItemState::Dropped(_))
                    && item.children.iter().any(|c| !c.done)
                {
                    out.push(Finding {
                        lint: "orphan-parent",
                        severity: Severity::Error,
                        subject: item.id.0.clone(),
                        message: "behind Now with unfinished children".into(),
                    });
                }

                if item
                    .children
                    .iter()
                    .any(|c| c.title.contains("@blocked(") || c.title.contains("@active"))
                {
                    out.push(Finding {
                        lint: "independent-children",
                        severity: Severity::Warning,
                        subject: item.id.0.clone(),
                        message: "children carrying status can be tracked separately; \
                                  they belong on the line"
                            .into(),
                    });
                }
            }
            Entry::Now => {}
        }
    }

    let now_count = line
        .entries
        .iter()
        .filter(|e| matches!(e, Entry::Now))
        .count();
    if now_count != 1 {
        out.push(Finding {
            lint: "no-now",
            severity: Severity::Error,
            subject: "NOW".into(),
            message: format!("expected exactly one NOW marker, found {now_count}"),
        });
    }

    let mut seen = std::collections::HashSet::new();
    for item in line.items() {
        if !seen.insert(item.id.0.clone()) {
            out.push(Finding {
                lint: "duplicate-id",
                severity: Severity::Error,
                subject: item.id.0.clone(),
                message: "two items share this id".into(),
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::format::parse;

    fn lints(src: &str) -> Vec<String> {
        check(&parse(src).unwrap(), &Config::default())
            .into_iter()
            .map(|f| f.lint.to_string())
            .collect()
    }

    #[test]
    fn a_bucket_marker_warns() {
        let out = check(
            &parse("# T\n\n── NOW ──\n\n◆ backlog ◆\n\n- [ ] a  ^aaa\n      body\n").unwrap(),
            &Config::default(),
        );
        let f = out
            .iter()
            .find(|f| f.lint == "bucket")
            .expect("no bucket lint");
        assert_eq!(f.severity, Severity::Warning);
    }

    #[test]
    fn the_allowlist_suppresses_the_bucket_lint() {
        let mut cfg = Config::default();
        cfg.check.allow_markers = vec!["v2".into()];
        let l = parse("# T\n\n── NOW ──\n\n◆ v2 ◆\n\n- [ ] a  ^aaa\n      body\n").unwrap();
        assert!(!check(&l, &cfg).iter().any(|f| f.lint == "bucket"));
    }

    #[test]
    fn a_bare_item_inside_the_window_is_unsharpened() {
        assert!(lints("# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n").contains(&"unsharpened".into()));
    }

    #[test]
    fn a_sharpened_item_inside_the_window_is_clean() {
        let out = lints("# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n      why this matters\n");
        assert!(!out.contains(&"unsharpened".into()));
    }

    #[test]
    fn a_long_body_far_from_now_is_false_certainty() {
        let mut src = String::from("# T\n\n── NOW ──\n\n");
        for n in 0..9 {
            src.push_str(&format!("- [ ] near{n}  ^n{n}\n      body\n"));
        }
        src.push_str("- [ ] distant  ^ddd\n      one\n      two\n      three\n      four\n");
        assert!(lints(&src).contains(&"false-certainty".into()));
    }

    #[test]
    fn results_do_not_count_toward_false_certainty() {
        // A long RESULT far behind Now is history and always allowed.
        let src = "# T\n\n- [x] a  ^aaa\n      → one\n        two\n        three\n        four\n\n── NOW ──\n\n- [ ] b  ^bbb\n      body\n";
        assert!(!lints(src).contains(&"false-certainty".into()));
    }

    #[test]
    fn a_result_ahead_of_now_is_an_error() {
        let src = "# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n      body\n      → done already\n";
        let out = check(&parse(src).unwrap(), &Config::default());
        let f = out
            .iter()
            .find(|f| f.lint == "result-ahead")
            .expect("no lint");
        assert_eq!(f.severity, Severity::Error);
    }

    #[test]
    fn a_parent_behind_now_with_open_children_is_an_orphan_parent() {
        let src = "# T\n\n- [x] a  ^aaa\n      - [ ] unfinished\n\n── NOW ──\n\n- [ ] b  ^bbb\n      body\n";
        assert!(lints(src).contains(&"orphan-parent".into()));
    }

    #[test]
    fn children_carrying_status_suggest_they_belong_on_the_line() {
        let src = "# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n      body\n      - [ ] one @blocked(keys)\n";
        assert!(lints(src).contains(&"independent-children".into()));
    }

    #[test]
    fn a_clean_line_produces_no_findings() {
        let src = "# T\n\n- [x] a  ^aaa\n      → shipped\n\n── NOW ──\n\n- [ ] b  ^bbb\n      why this matters\n";
        assert!(check(&parse(src).unwrap(), &Config::default()).is_empty());
    }

    #[test]
    fn warnings_alone_do_not_count_as_errors() {
        let findings = check(
            &parse("# T\n\n── NOW ──\n\n- [ ] a  ^aaa\n").unwrap(),
            &Config::default(),
        );
        assert!(!findings.is_empty());
        assert!(!has_errors(&findings));
    }
}

use crate::format::{parse, render};
use crate::model::Line;
use anyhow::{anyhow, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Walk up from `start` looking for `.throughline/line.md`.
pub fn find_line_file(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(".throughline/line.md");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

pub fn read(path: &Path) -> Result<Line> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse(&text).map_err(|e| {
        anyhow!(
            "{}:{}: {}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            e.line,
            e.message
        )
    })
}

/// Write to a temp file in the SAME directory, then rename. A rename within a
/// directory is atomic, so a crash mid-write can never truncate the line.
pub fn write_atomic(path: &Path, line: &Line) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).ok();
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(render(line).as_bytes())?;
    tmp.flush()?;
    tmp.persist(path)
        .map_err(|e| anyhow!("persisting {}: {}", path.display(), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::parse;

    #[test]
    fn finds_the_line_file_by_walking_up() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".throughline")).unwrap();
        std::fs::write(root.path().join(".throughline/line.md"), "# T\n\n── NOW ──\n").unwrap();
        let deep = root.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();

        let found = find_line_file(&deep).unwrap();
        assert_eq!(found, root.path().join(".throughline/line.md"));
    }

    #[test]
    fn returns_none_when_there_is_no_line_file() {
        let root = tempfile::tempdir().unwrap();
        assert!(find_line_file(root.path()).is_none());
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("line.md");
        let l = parse("# T\n\n- [x] a  ^aaa\n\n── NOW ──\n\n- [ ] b  ^bbb\n").unwrap();

        write_atomic(&path, &l).unwrap();
        assert_eq!(read(&path).unwrap(), l);
    }

    #[test]
    fn write_leaves_no_temp_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("line.md");
        let l = parse("# T\n\n── NOW ──\n").unwrap();

        write_atomic(&path, &l).unwrap();

        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["line.md".to_string()]);
    }

    #[test]
    fn a_parse_error_surfaces_the_path_and_line_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("line.md");
        std::fs::write(&path, "# T\n\n── NOW ──\n\n- [ ] no id\n").unwrap();

        let msg = read(&path).unwrap_err().to_string();
        assert!(msg.contains("line.md:5"), "got: {msg}");
    }
}

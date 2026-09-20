//! Golden reports for every example. The JSON is the stable contract; a
//! change here is either a deliberate improvement (review the diff, run
//! `cargo insta accept`) or a regression (fix it).

use pyxray_core::xray;

fn examples() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut out = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "py") {
                let stem = path
                    .strip_prefix(dir)
                    .unwrap()
                    .display()
                    .to_string()
                    .replace(['/', '\\'], "_");
                let parent = dir.file_name().unwrap().to_string_lossy().to_string();
                let name = if parent == "examples" {
                    stem
                } else {
                    format!("{parent}_{stem}")
                };
                let source = std::fs::read_to_string(&path)
                    .unwrap()
                    .replace("\r\n", "\n");
                out.push((name, source));
            }
        }
    }
    walk(&root, &mut out);
    out
}

#[test]
fn every_example_report_matches_its_snapshot() {
    let cases = examples();
    assert!(cases.len() >= 8, "found {}", cases.len());
    for (name, source) in cases {
        let file = name.trim_end_matches(".py").to_string();
        let report = xray(&source, &format!("{file}.py"));
        insta::assert_json_snapshot!(file, report);
    }
}

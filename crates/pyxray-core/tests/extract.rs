//! The extraction cases shared with the Python test-suite. One file, two
//! consumers, so the two sides cannot drift apart.

use std::path::PathBuf;

use pyxray_core::extract::{extract_all, ExtractOpts};

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    command: String,
    #[serde(default)]
    files: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    depth: Option<u8>,
    expect: Vec<Expect>,
}

#[derive(serde::Deserialize)]
struct Expect {
    source: String,
    label: String,
}

fn fixture() -> Vec<Case> {
    let text = include_str!("../../../tests/fixtures/extract_cases.json");
    serde_json::from_str(text).expect("fixture parses")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "pyxray-extract-{}-{}",
        std::process::id(),
        name.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn every_shared_case_extracts_as_expected() {
    let cases = fixture();
    assert!(cases.len() >= 40);
    for case in cases {
        let dir = scratch(&case.name);
        for (rel, body) in &case.files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, body).unwrap();
        }
        let opts = ExtractOpts {
            cwd: Some(dir.clone()),
            depth: case.depth.unwrap_or(1),
            ..ExtractOpts::default()
        };
        let got: Vec<(String, String)> = extract_all(&case.command, &opts)
            .into_iter()
            .map(|e| (e.source, e.label))
            .collect();
        let want: Vec<(String, String)> = case
            .expect
            .iter()
            .map(|e| (e.source.clone(), e.label.clone()))
            .collect();
        assert_eq!(got, want, "case {:?}: {:?}", case.name, case.command);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_script_path_is_reported_even_when_not_read() {
    let opts = ExtractOpts {
        read_files: false,
        cwd: Some(PathBuf::from("/work")),
        ..ExtractOpts::default()
    };
    let got = extract_all("python3 tools/run.py", &opts);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].source, "");
    assert_eq!(
        got[0].path.as_deref(),
        Some(std::path::Path::new("/work/tools/run.py"))
    );
    assert_eq!(got[0].segment, "python3 tools/run.py");
}

#[test]
fn oversized_scripts_are_labelled_not_read() {
    let dir = scratch("big");
    std::fs::write(dir.join("big.py"), "x = 1\n".repeat(1000)).unwrap();
    let opts = ExtractOpts {
        cwd: Some(dir.clone()),
        max_bytes: 100,
        ..ExtractOpts::default()
    };
    let got = extract_all("python3 big.py", &opts);
    assert_eq!(got[0].label, "big.py (too large)");
    assert!(got[0].source.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

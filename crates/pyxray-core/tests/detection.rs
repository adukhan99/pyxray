//! What the catalogue is expected to notice, and what it is expected to leave
//! alone. These are the claims the tool makes to a reader deciding whether to
//! run something, so they are worth pinning down.

use pyxray_core::model::{Effect, Severity};
use pyxray_core::xray;

fn effects(source: &str) -> Vec<(Effect, Severity, String, Option<String>)> {
    xray(source, "t")
        .effects
        .into_iter()
        .map(|h| (h.effect, h.severity, h.verb, h.target))
        .collect()
}

fn has(source: &str, effect: Effect) -> bool {
    xray(source, "t").effects.iter().any(|h| h.effect == effect)
}

#[test]
fn open_modes_decide_read_versus_write() {
    for (mode, verb) in [
        ("'r'", "read"),
        ("'rb'", "read"),
        ("'w'", "truncate"),
        ("'a'", "append"),
        ("'x'", "create"),
        ("'r+'", "update"),
    ] {
        let hits = effects(&format!("open('f', {mode})"));
        assert_eq!(hits.len(), 1, "mode {mode}");
        assert_eq!(hits[0].2, verb, "mode {mode}");
    }
    // The default is a read, and the path comes through as the target.
    let hits = effects("open('data.csv')");
    assert_eq!(hits[0].0, Effect::FsRead);
    assert_eq!(hits[0].3.as_deref(), Some("data.csv"));
}

#[test]
fn a_mode_given_by_keyword_counts_too() {
    let hits = effects("open('f', mode='w')");
    assert_eq!(hits[0].2, "truncate");
}

#[test]
fn destructive_calls_are_caution() {
    for source in [
        "import shutil\nshutil.rmtree('/data')",
        "import os\nos.remove('/data/x')",
        "from pathlib import Path\nPath('x').unlink()",
    ] {
        let hits = effects(source);
        let hit = hits
            .iter()
            .find(|h| h.0 == Effect::FsDelete)
            .unwrap_or_else(|| panic!("no delete found in:\n{source}"));
        assert_eq!(hit.1, Severity::Caution, "{source}");
    }
}

#[test]
fn a_string_constant_is_shown_rather_than_its_name() {
    let hits = effects("import shutil\nSTAGE = '/tmp/stage'\nshutil.rmtree(STAGE)");
    let hit = hits.iter().find(|h| h.0 == Effect::FsDelete).unwrap();
    assert_eq!(hit.3.as_deref(), Some("/tmp/stage"));
}

#[test]
fn rebinding_a_constant_stops_us_claiming_to_know_it() {
    let report = xray(
        "import shutil\nSTAGE = '/tmp/stage'\nSTAGE = compute()\nshutil.rmtree(STAGE)",
        "t",
    );
    let hit = report
        .effects
        .iter()
        .find(|h| h.effect == Effect::FsDelete)
        .unwrap();
    assert_eq!(hit.target.as_deref(), Some("STAGE"));
}

#[test]
fn a_command_list_reads_as_a_command_line() {
    let hits = effects("import subprocess\nsubprocess.run(['ls', '-la', '/tmp'])");
    let hit = hits.iter().find(|h| h.0 == Effect::Process).unwrap();
    assert_eq!(hit.3.as_deref(), Some("ls -la /tmp"));
}

#[test]
fn hazardous_strings_escalate_the_severity() {
    let hits = effects("import subprocess\nsubprocess.run(['rm', '-rf', '/'])");
    let hit = hits.iter().find(|h| h.0 == Effect::Process).unwrap();
    assert_eq!(hit.1, Severity::Caution);
}

#[test]
fn tls_verification_being_off_is_worth_saying() {
    let report = xray(
        "import requests\nrequests.get('https://h/', verify=False)",
        "t",
    );
    let hit = report
        .effects
        .iter()
        .find(|h| h.effect == Effect::Net)
        .unwrap();
    assert_eq!(hit.severity, Severity::Caution);
    assert!(hit.note.as_ref().unwrap().contains("verify=False"));
}

#[test]
fn a_path_built_with_a_slash_is_still_a_path() {
    assert!(has(
        "from pathlib import Path\nout = Path('r')\n(out / 'x.json').write_text('{}')",
        Effect::FsWrite
    ));
}

#[test]
fn a_distinctive_method_is_caught_even_on_an_unknown_receiver() {
    let hits = effects("stats = whatever()\nstats.to_csv('out.csv')");
    let hit = hits.iter().find(|h| h.0 == Effect::FsWrite).unwrap();
    assert_eq!(hit.3.as_deref(), Some("out.csv"));
}

#[test]
fn a_handle_carries_its_origin_into_the_write() {
    let hits = effects("fh = open('report.txt', 'w')\nfh.write('hello')");
    let write = hits.iter().find(|h| h.2 == "write").unwrap();
    assert_eq!(write.3.as_deref(), Some("open('report.txt', 'w')"));
}

#[test]
fn one_caution_always_reaches_the_check_band() {
    // The compact renders show a band, not a number. If a single destructive
    // call could still come out green, the whole visual language would be
    // lying at exactly the moment it matters.
    for source in [
        "import shutil\nshutil.rmtree('/data')",
        "import subprocess\nsubprocess.run(['ls'])",
        "eval(user_input)",
        "import pickle\npickle.loads(blob)",
    ] {
        let report = xray(source, "t");
        assert!(
            report.band() >= pyxray_core::model::Band::Check,
            "{source} landed in {:?} at risk {}",
            report.band(),
            report.metrics.risk
        );
    }
}

#[test]
fn one_notable_reaches_the_routine_band() {
    for source in [
        "import requests\nrequests.get('https://h/')",
        "import time\ntime.sleep(5)",
        "open('out.txt', 'w')",
    ] {
        let report = xray(source, "t");
        assert!(
            report.band() >= pyxray_core::model::Band::Routine,
            "{source} landed in {:?} at risk {}",
            report.band(),
            report.metrics.risk
        );
    }
}

#[test]
fn quiet_things_stay_quiet() {
    // Pure computation should score nothing worth warning about.
    let report = xray(
        "import math\nxs = [math.sin(i) for i in range(10)]\ntotal = sum(xs)\n",
        "t",
    );
    assert!(report.metrics.risk < 5, "risk was {}", report.metrics.risk);
    assert!(report.meta.synopsis.starts_with("no side effects"));
}

#[test]
fn fetching_and_executing_scores_higher_than_either_alone() {
    let fetch = xray("import requests\nrequests.get('https://h/')", "t")
        .metrics
        .risk;
    let run = xray("import subprocess\nsubprocess.run(['ls'])", "t")
        .metrics
        .risk;
    let both = xray(
        "import requests, subprocess\nr = requests.get('https://h/')\nsubprocess.run([r.text])",
        "t",
    )
    .metrics
    .risk;
    assert!(both > fetch + run - 10, "{both} vs {fetch}+{run}");
}

#[test]
fn third_party_imports_are_flagged_as_such() {
    let report = xray("import os\nimport numpy as np\nimport requests\n", "t");
    let names: Vec<(&str, bool)> = report
        .imports
        .iter()
        .map(|i| (i.module.as_str(), i.third_party))
        .collect();
    assert_eq!(names, [("os", false), ("numpy", true), ("requests", true)]);
}

#[test]
fn imports_inside_a_function_are_marked_deferred() {
    let report = xray("def f():\n    import numpy\n", "t");
    assert!(report.imports[0].deferred);
}

#[test]
fn a_block_does_not_claim_its_children_effects() {
    let report = xray("def f():\n    print(1)\n", "t");
    let def = &report.spine.children[0];
    assert!(def.own_effects.is_empty(), "{:?}", def.own_effects);
    assert!(def.effects.contains(Effect::Stdout));
}

#[test]
fn a_loop_header_keeps_only_its_own_effects() {
    let report = xray("import os\nfor p in os.listdir('.'):\n    print(p)\n", "t");
    let loop_node = &report.spine.children[1];
    assert!(loop_node.own_effects.contains(Effect::FsRead));
    assert!(!loop_node.own_effects.contains(Effect::Stdout));
    assert!(loop_node.effects.contains(Effect::Stdout));
}

#[test]
fn unused_bindings_are_called_out() {
    let report = xray("keep = 1\ndrop = 2\nprint(keep)\n", "t");
    let by = |name: &str| {
        report
            .bindings
            .iter()
            .find(|b| b.name == name)
            .unwrap()
            .dead
    };
    assert!(!by("keep"));
    assert!(by("drop"));
}

#[test]
fn the_call_inventory_drops_uninteresting_methods() {
    let report = xray(
        "import numpy as np\nxs = []\nxs.append(1)\nnp.mean(xs)\n",
        "t",
    );
    let names: Vec<&str> = report
        .symbols
        .iter()
        .map(|s| s.canonical.as_str())
        .collect();
    assert!(names.contains(&"numpy.mean"), "{names:?}");
    assert!(!names.contains(&"xs.append"), "{names:?}");
}

#[test]
fn the_timeline_folds_repeats_and_keeps_the_worst() {
    let report = xray(
        "import shutil\nprint(1)\nprint(2)\nprint(3)\nprint(4)\nshutil.rmtree('/x')\n",
        "t",
    );
    let rows = report.timeline(2);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|(h, _)| h.effect == Effect::FsDelete));
}

#[test]
fn a_heredoc_with_tabs_and_a_bare_delimiter_still_parses() {
    let (code, label) = pyxray_core::extract_python("python3 <<-PY\nprint(1)\n\tPY\n");
    assert_eq!(code, "print(1)\n");
    assert!(label.contains("PY"), "{label}");
}

#[test]
fn something_that_is_not_a_wrapped_command_comes_back_untouched() {
    let (code, label) = pyxray_core::extract_python("print('already python')\n");
    assert_eq!(code, "print('already python')\n");
    assert_eq!(label, "<stdin>");
}

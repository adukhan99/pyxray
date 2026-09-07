//! pyxray-core — parse a Python snippet and work out what it will actually do.
//!
//! ```no_run
//! let report = pyxray_core::xray("print('hi')", "<stdin>");
//! println!("{}", report.meta.synopsis);
//! ```
//!
//! The output is a [`Report`]: a structural spine, a list of side effects with
//! severities, the names it binds, the symbols it leans on, and a per-line
//! strip for minimaps. Nothing in here knows how any of it is drawn.

pub mod analyze;
pub mod digest;
pub mod effects;
pub mod model;
pub mod source;

pub use model::*;

use ruff_python_parser::{parse_unchecked, ParseOptions};
use source::Src;

/// Analyse a snippet. Never fails: a snippet that does not parse still yields
/// a report built from whatever the recovering parser salvaged, with the
/// syntax errors attached as diagnostics — which is the useful behaviour when
/// the input came from a language model rather than a file on disk.
pub fn xray(source: &str, name: &str) -> Report {
    let src = Src::new(source);
    let parsed = parse_unchecked(source, ParseOptions::from(ruff_python_parser::Mode::Module));

    let mut diagnostics: Vec<Diagnostic> = parsed
        .errors()
        .iter()
        .map(|e| Diagnostic {
            level: DiagLevel::Error,
            message: e.error.to_string(),
            line: src.line_of(e.location.start()),
            col: src.col_of(e.location.start()),
        })
        .collect();
    let parsed_ok = diagnostics.is_empty();

    let body: &[ruff_python_ast::Stmt] = match parsed.syntax() {
        ruff_python_ast::Mod::Module(m) => &m.body,
        ruff_python_ast::Mod::Expression(_) => &[],
    };

    let mut an = analyze::Analyzer::new(&src);
    an.prescan(body, false);
    let children = an.body(body, 0);

    let mut spine = Node {
        id: 0,
        kind: NodeKind::Module,
        label: name.to_string(),
        detail: None,
        line: 1,
        end_line: src.line_count(),
        depth: 0,
        own_effects: EffectMask::default(),
        effects: EffectMask::default(),
        severity: None,
        weight: 1,
        children,
    };
    for c in &spine.children {
        spine.effects.union(c.effects);
        if let Some(cs) = c.severity {
            spine.severity = Some(spine.severity.map_or(cs, |s: Severity| s.max(cs)));
        }
        spine.weight += c.weight;
    }

    let mut found = an.into_parts();
    diagnostics.append(&mut found.diagnostics);
    let analyze::Findings {
        imports,
        effects: mut hits,
        mut symbols,
        mut bindings,
        mut metrics,
        reads,
        max_depth,
        ..
    } = found;

    digest::dedup(&mut hits);
    hits.sort_by_key(|h| (h.line, h.col));

    // Recompute node effect masks from the deduplicated hit list would be
    // exact, but dedup only ever removes duplicates of effects already in the
    // mask, so the masks stay correct as they are.

    for b in &mut bindings {
        b.reads = reads.get(&b.name).copied().unwrap_or(0);
        // The binding's own name appears once on the left-hand side, which is
        // a store, not a read — so any count at all means it is used.
        b.dead = b.reads == 0;
    }

    // The call inventory is for "what does this lean on", so it keeps library
    // calls, builtins and the file's own functions. Methods on values we could
    // not identify are noise unless they turned out to do something.
    symbols.retain(|s| match s.origin {
        SymbolOrigin::Builtin => !analyze::is_noise_symbol(&s.canonical),
        SymbolOrigin::Method => !s.effects.is_empty(),
        SymbolOrigin::Import | SymbolOrigin::Local => true,
    });

    let (total, code, comment, blank) = digest::line_counts(&src);
    metrics.lines_total = total;
    metrics.lines_code = code;
    metrics.lines_comment = comment;
    metrics.lines_blank = blank;
    metrics.max_depth = max_depth as u32;
    metrics.complexity += 1;
    metrics.risk = digest::risk(&hits);

    let texture = digest::texture(&src, &spine, &hits);
    let (synopsis, headline) = if source.trim().is_empty() {
        // `LineIndex` counts an empty string as one line, which would have the
        // header claim there is something here.
        metrics.lines_total = 0;
        metrics.lines_code = 0;
        ("nothing to run".to_string(), Vec::new())
    } else {
        digest::synopsis(&hits, &metrics, &spine)
    };

    Report {
        meta: Meta {
            name: name.to_string(),
            bytes: source.len(),
            parsed: parsed_ok,
            synopsis,
            headline,
        },
        spine,
        imports,
        effects: hits,
        symbols,
        bindings,
        metrics,
        texture,
        diagnostics,
        source: source.lines().map(|l| l.to_string()).collect(),
    }
}

/// Pull the Python body out of the shell command a model typically emits —
/// `python3 <<'EOF' … EOF`, `python -c '…'`, or a bare script. Returns the
/// source and a label describing where it came from.
pub fn extract_python(input: &str) -> (String, String) {
    let trimmed = input.trim_start();

    // heredoc: python3 <<'PY' … PY
    if let Some(rest) = strip_python_prefix(trimmed) {
        if let Some(idx) = rest.find("<<") {
            let after = &rest[idx + 2..];
            let after = after.strip_prefix('-').unwrap_or(after);
            let after = after.trim_start();
            let (delim, body_start) = read_delimiter(after);
            if let Some(delim) = delim {
                let body = &after[body_start..];
                let body = body.strip_prefix('\n').unwrap_or(body);
                if let Some(end) = find_terminator(body, &delim) {
                    return (body[..end].to_string(), format!("heredoc <<{delim}"));
                }
                return (
                    body.to_string(),
                    format!("heredoc <<{delim} (unterminated)"),
                );
            }
        }
        // python -c "…"
        for flag in ["-c ", "-c'", "-c\""] {
            if let Some(pos) = rest.find(flag) {
                let after = rest[pos + 2..].trim_start();
                if let Some(code) = unquote(after) {
                    return (code, "python -c".to_string());
                }
            }
        }
    }
    (input.to_string(), "<stdin>".to_string())
}

fn strip_python_prefix(s: &str) -> Option<&str> {
    for prefix in ["python3 ", "python ", "python3.", "uv run python", "py "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return Some(rest);
        }
    }
    None
}

/// Read a heredoc delimiter, quoted or bare, returning it plus the offset just
/// past it.
fn read_delimiter(s: &str) -> (Option<String>, usize) {
    let bytes = s.as_bytes();
    match bytes.first() {
        Some(&q @ (b'\'' | b'"')) => {
            let rest = &s[1..];
            match rest.find(q as char) {
                Some(end) => (Some(rest[..end].to_string()), end + 2),
                None => (None, 0),
            }
        }
        Some(_) => {
            let end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
            let word = &s[..end];
            if word.is_empty() {
                (None, 0)
            } else {
                (Some(word.to_string()), end)
            }
        }
        None => (None, 0),
    }
}

/// A heredoc ends at a line that is exactly the delimiter, allowing for the
/// leading tabs that `<<-` permits.
fn find_terminator(body: &str, delim: &str) -> Option<usize> {
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if line.trim_end_matches(['\n', '\r']).trim_start_matches('\t') == delim {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

fn unquote(s: &str) -> Option<String> {
    let mut chars = s.chars();
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &s[quote.len_utf8()..];
    let end = rest.rfind(quote)?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_are_distinguished() {
        let r = xray("f = open('a.txt')\ng = open('b.txt', 'w')\n", "t");
        let verbs: Vec<&str> = r.effects.iter().map(|e| e.verb.as_str()).collect();
        assert!(verbs.contains(&"read"), "{verbs:?}");
        assert!(verbs.contains(&"truncate"), "{verbs:?}");
    }

    #[test]
    fn aliases_resolve() {
        let r = xray("import numpy as np\nx = np.load('a.npy')\n", "t");
        assert!(r.effects.iter().any(|e| e.symbol == "numpy.load"));
    }

    #[test]
    fn method_calls_on_inferred_values_resolve() {
        let r = xray(
            "from pathlib import Path\nPath('x').write_text('hi')\n",
            "t",
        );
        assert!(
            r.effects.iter().any(|e| e.effect == Effect::FsWrite),
            "{:?}",
            r.effects
        );
    }

    #[test]
    fn shell_true_escalates() {
        let r = xray("import subprocess\nsubprocess.run('ls', shell=True)\n", "t");
        let hit = r
            .effects
            .iter()
            .find(|e| e.effect == Effect::Process)
            .unwrap();
        assert_eq!(hit.severity, Severity::Caution);
        assert!(hit.note.as_ref().unwrap().contains("shell=True"));
    }

    #[test]
    fn user_functions_do_not_impersonate_builtins() {
        let r = xray("def open(p):\n    return p\nopen('x')\n", "t");
        assert!(r.effects.is_empty(), "{:?}", r.effects);
    }

    #[test]
    fn broken_source_still_reports() {
        let r = xray("def f(:\n    pass\n", "t");
        assert!(!r.meta.parsed);
        assert!(!r.diagnostics.is_empty());
    }

    #[test]
    fn heredoc_is_extracted() {
        let (code, label) = extract_python("python3 <<'EOF'\nprint(1)\nEOF\n");
        assert_eq!(code, "print(1)\n");
        assert!(label.contains("EOF"));
    }

    #[test]
    fn dash_c_is_extracted() {
        let (code, _) = extract_python("python3 -c 'print(1)'");
        assert_eq!(code, "print(1)");
    }

    #[test]
    fn accumulators_are_spotted() {
        let r = xray("total = 0\nfor n in [1,2]:\n    total += n\n", "t");
        let b = r.bindings.iter().find(|b| b.name == "total").unwrap();
        assert!(b.accumulates);
    }
}

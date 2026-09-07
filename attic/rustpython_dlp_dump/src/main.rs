//! dlp_dump — render a RustPython (rustpython-parser 0.4.0) AST in a compact,
//! indentation-based ".dlp" layout (one line per node, children indented below).
//!
//! Usage:
//!   dlp_dump <file.py> [--mode module|interactive|expression]
//!   cat file.py | dlp_dump            # reads source from stdin
//!
//! The dump is printed to stdout and also written to a sidecar
//! "<input>.dlp" (or "stdin.dlp" for pipes).

use rustpython_parser::ast as ast;
use rustpython_parser::{parse_starts_at, Mode, Parse};
use rustpython_parser::text_size::{TextRange, TextSize};
use rustpython_ast::Visitor;

use ast::*;

mod overrides;

const INDENT_WIDTH: usize = 2;

/// Indentation-based AST dumper.
pub struct DlpDumper {
    out: String,
    indent: usize,
    source: String,
    /// Offset where the snippet being dumped starts in `source`
    /// (so a sub-`Mod` parsed with `parse_starts_at` points back correctly).
    base: TextSize,
}

impl DlpDumper {
    fn new(source: String) -> Self {
        Self {
            out: String::new(),
            indent: 0,
            source,
            base: TextSize::from(0),
        }
    }

    /// Map a `TextRange`'s start offset to `SourceLocation` (1-based row, 0-based col).
    fn locate(&self, range: TextRange) -> ast::source_code::SourceLocation {
        let line_index = ast::source_code::LineIndex::from_source_text(&self.source);
        line_index.source_location(range.start(), &self.source)
    }

    fn emit(&mut self, label: &str, range: TextRange, args: Option<&[std::fmt::Arguments]>) {
        let pad = " ".repeat(self.indent * INDENT_WIDTH);
        let loc = self.locate(range);
        let args = match args {
            Some(parts) if !parts.is_empty() => {
                let joined: Vec<String> = parts.iter().map(|a| format!("{}", a)).collect();
                format!(" ({})", joined.join(", "))
            }
            _ => String::new(),
        };
        let _ = std::fmt::write(
            &mut self.out,
            format_args!("{pad}{label} @ L{}:{}{args}\n", loc.row, loc.column),
        );
    }

    fn result(self) -> String {
        self.out
    }

    fn run(mut self, module: ast::Mod) -> String {
        // The `Visitor` trait has no `visit_mod` (the `Mod` enum is the entry
        // point, not a node), so we dispatch on its variants and walk the
        // children ourselves using the trait's `visit_stmt` / `visit_expr`.
        let range = module.range();
        match module {
            ast::Mod::Module(mut m) => {
                self.emit("Module", range, None);
                self.indent += 1;
                for stmt in std::mem::take(&mut m.body) {
                    self.visit_stmt(stmt);
                }
                // `type_ignores` has no visitor method; skip it.
                let _ = std::mem::take(&mut m.type_ignores);
                self.indent -= 1;
            }
            ast::Mod::Interactive(mut m) => {
                self.emit("Interactive", range, None);
                self.indent += 1;
                for stmt in std::mem::take(&mut m.body) {
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
            }
            ast::Mod::Expression(mut m) => {
                self.emit("Expression", range, None);
                self.indent += 1;
                self.visit_expr(*m.body);
                self.indent -= 1;
            }
            ast::Mod::FunctionType(mut m) => {
                self.emit("FunctionType", range, None);
                self.indent += 1;
                for argtype in std::mem::take(&mut m.argtypes) {
                    self.visit_expr(argtype);
                }
                self.visit_expr(*m.returns);
                self.indent -= 1;
            }
        }
        self.result()
    }
}

#[allow(clippy::too_many_arguments)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut positional: Vec<String> = Vec::new();
    let mut mode = Mode::Module;

    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--mode" => {
                i += 1;
                mode = match args.get(i).map(|s| s.as_str()) {
                    Some("module") => Mode::Module,
                    Some("interactive") => Mode::Interactive,
                    Some("expression") => Mode::Expression,
                    other => {
                        eprintln!("dlp_dump: unknown --mode {:?} (use module|interactive|expression)", other);
                        std::process::exit(2);
                    }
                };
            }
            _ if a.starts_with("--mode=") => {
                let v = &a["--mode=".len()..];
                mode = match v {
                    "module" => Mode::Module,
                    "interactive" => Mode::Interactive,
                    "expression" => Mode::Expression,
                    other => {
                        eprintln!("dlp_dump: unknown --mode {:?}", other);
                        std::process::exit(2);
                    }
                };
            }
            _ if a.starts_with('-') && a != "-" => {
                eprintln!("dlp_dump: unknown flag {a}");
                std::process::exit(2);
            }
            _ => positional.push(a.clone()),
        }
        i += 1;
    }

    let (source, out_path): (String, Option<String>) = if let Some(path) = positional.first() {
        match std::fs::read_to_string(path) {
            Ok(s) => (s, Some(format!("{path}.dlp"))),
            Err(e) => {
                eprintln!("dlp_dump: cannot read {path}: {e}");
                std::process::exit(1);
            }
        }
    } else {
        let mut buf = String::new();
        if let Err(e) = std::io::read_to_string(&mut std::io::stdin()).map(|s| buf = s) {
            eprintln!("dlp_dump: stdin read error: {e}");
            std::process::exit(1);
        }
        (buf, Some("stdin.dlp".to_string()))
    };

    let module = match mode {
        Mode::Expression => {
            let expr = ast::Expr::parse(&source, "<expr>")
                .unwrap_or_else(|e| die(&source, e));
            ast::Mod::Expression(ast::ModExpression {
                range: Default::default(),
                body: Box::new(expr),
            })
        }
        Mode::Interactive => parse_starts_at(&source, Mode::Interactive, "<input>", TextSize::from(0))
            .unwrap_or_else(|e| die(&source, e)),
        Mode::Module => {
            let suite = ast::Suite::parse(&source, "<input>")
                .unwrap_or_else(|e| die(&source, e));
            ast::Mod::Module(ast::ModModule {
                range: Default::default(),
                body: suite,
                type_ignores: Vec::new(),
            })
        }
    };

    let dumper = DlpDumper::new(source.clone());
    let dump = dumper.run(module);

    print!("{dump}");
    if let Some(p) = out_path {
        if let Err(e) = std::fs::write(&p, &dump) {
            eprintln!("dlp_dump: warning: could not write {p}: {e}");
        } else {
            eprintln!("dlp_dump: wrote {p}");
        }
    }
}

fn die(source: &str, e: rustpython_parser::ParseError) -> ! {
    let line_index = ast::source_code::LineIndex::from_source_text(source);
    let loc = line_index.source_location(e.offset, source);
    eprintln!("dlp_dump: parse error at L{}:{}: {}", loc.row, loc.column, e);
    std::process::exit(1);
}

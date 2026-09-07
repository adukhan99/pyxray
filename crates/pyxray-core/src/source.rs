//! Source-text helpers. Labels in the report are mostly *slices of the user's
//! own code* rather than reconstructions — that is both far less code than a
//! pretty-printer and far more faithful to what they wrote. We only fall back
//! to a structural summary when the real thing is too big to show.

use ruff_python_ast::{Expr, Stmt};
use ruff_source_file::{LineIndex, OneIndexed};
use ruff_text_size::{Ranged, TextRange, TextSize};

pub struct Src<'a> {
    pub text: &'a str,
    pub index: LineIndex,
    pub lines: Vec<&'a str>,
}

impl<'a> Src<'a> {
    pub fn new(text: &'a str) -> Self {
        Src {
            index: LineIndex::from_source_text(text),
            lines: text.lines().collect(),
            text,
        }
    }

    pub fn slice(&self, range: TextRange) -> &'a str {
        let start = usize::from(range.start()).min(self.text.len());
        let end = usize::from(range.end()).min(self.text.len());
        if start > end {
            return "";
        }
        // Snap to char boundaries so a range that lands mid-codepoint (which
        // shouldn't happen, but a recovering parser can produce odd spans)
        // never panics.
        let mut s = start;
        while s < self.text.len() && !self.text.is_char_boundary(s) {
            s += 1;
        }
        let mut e = end;
        while e < self.text.len() && !self.text.is_char_boundary(e) {
            e += 1;
        }
        &self.text[s..e]
    }

    pub fn line_of(&self, offset: TextSize) -> u32 {
        self.index.line_index(offset).get() as u32
    }

    pub fn col_of(&self, offset: TextSize) -> u32 {
        self.index.line_column(offset, self.text).column.get() as u32
    }

    pub fn line_count(&self) -> u32 {
        self.index.line_count() as u32
    }

    pub fn line_text(&self, line: u32) -> &'a str {
        self.lines
            .get(line.saturating_sub(1) as usize)
            .copied()
            .unwrap_or("")
    }

    pub fn line_start(&self, line: u32) -> TextSize {
        self.index.line_start(
            OneIndexed::from_zero_indexed(line.saturating_sub(1) as usize),
            self.text,
        )
    }
}

/// Collapse every run of whitespace (including newlines and the indentation
/// that follows them) to a single space.
pub fn squeeze(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            out.push(ch);
        }
    }
    out
}

/// Truncate to `budget` *characters* (not bytes), appending an ellipsis.
pub fn clip(s: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= budget {
        return s.to_string();
    }
    let keep = budget.saturating_sub(1);
    let mut out: String = s.chars().take(keep).collect();
    // Prefer to cut at a token boundary so we don't end mid-identifier.
    if let Some(cut) = out.rfind([' ', ',', '(', '[']) {
        if cut > keep * 2 / 3 {
            out.truncate(cut);
        }
    }
    out.push('…');
    out
}

/// A short, faithful label for an expression: its own source when that is
/// small enough, otherwise a shape summary.
pub fn brief_expr(src: &Src, expr: &Expr, budget: usize) -> String {
    let raw = squeeze(src.slice(expr.range()));
    if raw.chars().count() <= budget {
        return raw;
    }
    let shaped = shape_expr(src, expr, budget);
    if shaped.chars().count() <= budget {
        shaped
    } else {
        clip(&shaped, budget)
    }
}

/// Structural stand-in for an expression too long to show verbatim.
pub fn shape_expr(src: &Src, expr: &Expr, budget: usize) -> String {
    match expr {
        Expr::Dict(d) => plural(d.items.len(), "key", "keys", '{', '}'),
        Expr::Set(s) => plural(s.elts.len(), "item", "items", '{', '}'),
        Expr::List(l) => plural(l.elts.len(), "item", "items", '[', ']'),
        Expr::Tuple(t) => plural(t.elts.len(), "item", "items", '(', ')'),
        Expr::ListComp(_) => "[… for …]".into(),
        Expr::SetComp(_) => "{… for …}".into(),
        Expr::DictComp(_) => "{…: … for …}".into(),
        Expr::Generator(_) => "(… for …)".into(),
        Expr::Lambda(l) => match &l.parameters {
            Some(p) => format!("lambda {}", param_names(p).join(", ")),
            None => "lambda".into(),
        },
        Expr::FString(_) => "f\"…\"".into(),
        Expr::StringLiteral(s) => {
            let v = s.value.to_str();
            format!("{:?}", clip(&squeeze(v), budget.saturating_sub(2)))
        }
        Expr::BytesLiteral(_) => "b\"…\"".into(),
        Expr::Call(c) => {
            // The callee and its first argument carry nearly all the meaning;
            // the rest becomes a count so the width stays predictable.
            let name = dotted(&c.func).unwrap_or_else(|| chain_shape(src, &c.func, 28));
            let extra = c.arguments.args.len() + c.arguments.keywords.len();
            match c.arguments.args.first() {
                None if extra == 0 => format!("{name}()"),
                None => format!("{name}(\u{2026}{extra})"),
                Some(first) => {
                    let room = budget.saturating_sub(name.chars().count() + 6).max(8);
                    let head = brief_expr(src, first, room);
                    if extra > 1 {
                        format!("{name}({head}, \u{2026}{})", extra - 1)
                    } else {
                        format!("{name}({head})")
                    }
                }
            }
        }
        Expr::BinOp(b) => format!(
            "{} {} {}",
            brief_expr(src, &b.left, budget / 3),
            b.op.as_str(),
            brief_expr(src, &b.right, budget / 3)
        ),
        Expr::Compare(c) => {
            let op = c.ops.first().map(|o| o.as_str()).unwrap_or("?");
            let rhs = c
                .comparators
                .first()
                .map(|e| brief_expr(src, e, budget / 3))
                .unwrap_or_default();
            format!("{} {} {}", brief_expr(src, &c.left, budget / 3), op, rhs)
        }
        Expr::BoolOp(b) => {
            let op = if b.op.is_and() { "and" } else { "or" };
            format!("… {op} … ({} terms)", b.values.len())
        }
        Expr::Subscript(s) => format!("{}[…]", brief_expr(src, &s.value, budget.saturating_sub(3))),
        Expr::Attribute(_) | Expr::Name(_) => {
            dotted(expr).unwrap_or_else(|| clip(&squeeze(src.slice(expr.range())), budget))
        }
        Expr::If(_) => "… if … else …".into(),
        Expr::Await(a) => format!(
            "await {}",
            brief_expr(src, &a.value, budget.saturating_sub(6))
        ),
        _ => clip(&squeeze(src.slice(expr.range())), budget),
    }
}

/// A readable stand-in for a callee that is not a plain dotted name:
/// `merged.groupby(...)["rmsf"].agg` becomes `merged.groupby(…)[…].agg`, which
/// keeps the shape of the chain without any of its bulk.
pub fn chain_shape(src: &Src, expr: &Expr, budget: usize) -> String {
    let text = match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Attribute(a) => format!("{}.{}", chain_shape(src, &a.value, budget), a.attr),
        Expr::Call(c) => {
            let inner = chain_shape(src, &c.func, budget);
            if c.arguments.args.is_empty() && c.arguments.keywords.is_empty() {
                format!("{inner}()")
            } else {
                format!("{inner}(\u{2026})")
            }
        }
        Expr::Subscript(s) => format!("{}[\u{2026}]", chain_shape(src, &s.value, budget)),
        other => clip(&squeeze(src.slice(other.range())), budget),
    };
    clip(&text, budget)
}

fn plural(n: usize, one: &str, many: &str, open: char, close: char) -> String {
    let word = if n == 1 { one } else { many };
    format!("{open}{n} {word}{close}")
}

pub fn param_names(p: &ruff_python_ast::Parameters) -> Vec<String> {
    let mut out = Vec::new();
    for a in &p.posonlyargs {
        out.push(a.parameter.name.to_string());
    }
    if !p.posonlyargs.is_empty() {
        out.push("/".into());
    }
    for a in &p.args {
        out.push(a.parameter.name.to_string());
    }
    if let Some(v) = &p.vararg {
        out.push(format!("*{}", v.name));
    } else if !p.kwonlyargs.is_empty() {
        out.push("*".into());
    }
    for a in &p.kwonlyargs {
        out.push(a.parameter.name.to_string());
    }
    if let Some(k) = &p.kwarg {
        out.push(format!("**{}", k.name));
    }
    out
}

/// Flatten a `a.b.c` attribute/name chain into a dotted string. Returns `None`
/// for anything with a call, subscript or literal in the middle, which is
/// exactly the case where a dotted path would be a lie.
pub fn dotted(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(n) => Some(n.id.to_string()),
        Expr::Attribute(a) => dotted(&a.value).map(|base| format!("{base}.{}", a.attr)),
        _ => None,
    }
}

/// Like [`dotted`], but tolerates one call or subscript at the base and
/// reports it: `Path(p).write_text` yields `("Path", ["write_text"])`.
pub fn dotted_through_call(expr: &Expr) -> Option<(String, Vec<String>)> {
    fn walk(expr: &Expr, tail: &mut Vec<String>) -> Option<String> {
        match expr {
            Expr::Name(n) => Some(n.id.to_string()),
            Expr::Attribute(a) => {
                tail.push(a.attr.to_string());
                walk(&a.value, tail)
            }
            Expr::Call(c) => dotted(&c.func),
            Expr::Subscript(s) => walk(&s.value, tail),
            _ => None,
        }
    }
    let mut tail = Vec::new();
    let base = walk(expr, &mut tail)?;
    tail.reverse();
    Some((base, tail))
}

/// The first physical line a statement occupies, ignoring decorators — those
/// are listed separately and would otherwise drag the reported line upwards.
pub fn stmt_line(src: &Src, stmt: &Stmt) -> u32 {
    let offset = match stmt {
        Stmt::FunctionDef(f) => f
            .decorator_list
            .last()
            .map(|d| d.range().end())
            .unwrap_or(f.range.start()),
        Stmt::ClassDef(c) => c
            .decorator_list
            .last()
            .map(|d| d.range().end())
            .unwrap_or(c.range.start()),
        other => other.range().start(),
    };
    src.line_of(offset)
}

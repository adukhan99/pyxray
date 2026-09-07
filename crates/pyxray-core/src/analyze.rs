//! Walks the AST once and produces everything in [`Report`].
//!
//! The walk is deliberately hand-written rather than driven by the visitor
//! trait: the spine needs to know, at every point, which statements are
//! *children* of the current one and which expressions belong to its header,
//! and that distinction is the whole shape of the output.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{
    ExceptHandler, Expr, ExprAttribute, ExprCall, ExprContext, ExprSubscript, Stmt,
};
use ruff_text_size::Ranged;

use crate::effects::{self, TargetArg};
use crate::model::*;
use crate::source::{brief_expr, clip, param_names, squeeze, stmt_line, Src};

const LABEL: usize = 64;
const TARGET: usize = 44;
const ORIGIN: usize = 40;

pub struct Analyzer<'a> {
    src: &'a Src<'a>,
    /// Local name → canonical dotted path it stands for (`np` → `numpy`).
    aliases: HashMap<String, String>,
    /// Local name → what we think it holds, for resolving method calls.
    kinds: HashMap<String, ValueKind>,
    /// Local name → the string it was assigned, so `rmtree(STAGING)` can show
    /// the path it is actually about to delete.
    consts: HashMap<String, String>,
    /// Local name → a short rendering of what it was bound to, used as the
    /// target when a method call has no argument that names one.
    origins: HashMap<String, String>,
    /// Root modules reached by an import, for telling a library call apart
    /// from a method on some local object.
    imported_roots: HashSet<String>,
    /// Names bound by `def`/`class`/assignment in this file, so we don't
    /// attribute a builtin's behaviour to a user's own function.
    locals: HashSet<String>,
    imports: Vec<ImportInfo>,
    effects: Vec<EffectHit>,
    symbols: HashMap<String, SymbolUse>,
    bindings: Vec<Binding>,
    binding_at: HashMap<String, usize>,
    reads: HashMap<String, u32>,
    diagnostics: Vec<Diagnostic>,
    metrics: Metrics,
    scope: Vec<String>,
    loop_depth: u32,
    next_id: u32,
    max_depth: u16,
}

impl<'a> Analyzer<'a> {
    pub fn new(src: &'a Src<'a>) -> Self {
        Analyzer {
            src,
            aliases: HashMap::new(),
            kinds: HashMap::new(),
            consts: HashMap::new(),
            origins: HashMap::new(),
            imported_roots: HashSet::new(),
            locals: HashSet::new(),
            imports: Vec::new(),
            effects: Vec::new(),
            symbols: HashMap::new(),
            bindings: Vec::new(),
            binding_at: HashMap::new(),
            reads: HashMap::new(),
            diagnostics: Vec::new(),
            metrics: Metrics::default(),
            scope: vec!["<module>".into()],
            loop_depth: 0,
            next_id: 0,
            max_depth: 0,
        }
    }

    fn id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    fn scope_name(&self) -> String {
        if self.scope.len() == 1 {
            "<module>".into()
        } else {
            self.scope[1..].join(".")
        }
    }

    // ---------------------------------------------------------------- pass 1

    /// Collect imports and top-level names before the main walk, so a symbol
    /// used above its own import (or shadowing a builtin) still resolves.
    pub fn prescan(&mut self, body: &[Stmt], deferred: bool) {
        for stmt in body {
            match stmt {
                Stmt::Import(imp) => {
                    for alias in &imp.names {
                        let module = alias.name.to_string();
                        let local =
                            alias
                                .asname
                                .as_ref()
                                .map(|a| a.to_string())
                                .unwrap_or_else(|| {
                                    module.split('.').next().unwrap_or(&module).to_string()
                                });
                        let target = match &alias.asname {
                            Some(_) => module.clone(),
                            // `import os.path` binds `os`, not `os.path`.
                            None => module.split('.').next().unwrap_or(&module).to_string(),
                        };
                        self.imported_roots
                            .insert(target.split('.').next().unwrap_or(&target).to_string());
                        self.aliases.insert(local.clone(), target);
                        self.imports.push(ImportInfo {
                            module: module.clone(),
                            names: Vec::new(),
                            alias: alias.asname.as_ref().map(|a| a.to_string()),
                            line: self.src.line_of(stmt.range().start()),
                            third_party: !effects::is_stdlib(&module),
                            deferred,
                        });
                    }
                }
                Stmt::ImportFrom(imp) => {
                    let module = match &imp.module {
                        Some(m) => format!("{}{m}", ".".repeat(imp.level as usize)),
                        None => ".".repeat(imp.level.max(1) as usize),
                    };
                    let mut names = Vec::new();
                    for alias in &imp.names {
                        let name = alias.name.to_string();
                        let local = alias
                            .asname
                            .as_ref()
                            .map(|a| a.to_string())
                            .unwrap_or_else(|| name.clone());
                        if name != "*" {
                            self.imported_roots
                                .insert(module.split('.').next().unwrap_or(&module).to_string());
                            self.aliases
                                .insert(local.clone(), format!("{module}.{name}"));
                        }
                        names.push(match &alias.asname {
                            Some(a) => format!("{name} as {a}"),
                            None => name,
                        });
                    }
                    self.imports.push(ImportInfo {
                        module: module.clone(),
                        names,
                        alias: None,
                        line: self.src.line_of(stmt.range().start()),
                        third_party: imp.level == 0 && !effects::is_stdlib(&module),
                        deferred,
                    });
                }
                Stmt::FunctionDef(f) => {
                    self.locals.insert(f.name.to_string());
                    self.prescan(&f.body, true);
                }
                Stmt::ClassDef(c) => {
                    self.locals.insert(c.name.to_string());
                    self.prescan(&c.body, true);
                }
                Stmt::If(s) => {
                    self.prescan(&s.body, deferred);
                    for clause in &s.elif_else_clauses {
                        self.prescan(&clause.body, deferred);
                    }
                }
                Stmt::Try(s) => {
                    self.prescan(&s.body, true);
                    for handler in &s.handlers {
                        let ExceptHandler::ExceptHandler(h) = handler;
                        self.prescan(&h.body, true);
                    }
                    self.prescan(&s.orelse, true);
                    self.prescan(&s.finalbody, true);
                }
                Stmt::For(s) => self.prescan(&s.body, deferred),
                Stmt::While(s) => self.prescan(&s.body, deferred),
                Stmt::With(s) => self.prescan(&s.body, deferred),
                Stmt::Assign(s) => {
                    for t in &s.targets {
                        if let Expr::Name(n) = t {
                            self.locals.insert(n.id.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // ------------------------------------------------------------ resolution

    /// The canonical namespace of an expression's *value*: what you would have
    /// to write, with no aliases in play, to name the same thing.
    fn resolve(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Name(n) => {
                let id = n.id.as_str();
                if let Some(kind) = self.kinds.get(id) {
                    if let Some(prefix) = effects::kind_prefix(*kind) {
                        return Some(prefix.to_string());
                    }
                }
                Some(
                    self.aliases
                        .get(id)
                        .cloned()
                        .unwrap_or_else(|| id.to_string()),
                )
            }
            Expr::Attribute(a) => self
                .resolve(&a.value)
                .map(|base| format!("{base}.{}", a.attr)),
            Expr::Call(c) => {
                let base = self.resolve(&c.func)?;
                match effects::kind_prefix(effects::produces(&base)) {
                    Some(prefix) => Some(prefix.to_string()),
                    None => Some(base),
                }
            }
            // `paths[0].read_text()` and `os.environ["X"]` both want the
            // container's identity, not the element's.
            Expr::Subscript(s) => self.resolve(&s.value),
            Expr::Await(a) => self.resolve(&a.value),
            // `out / "flexible.json"` is still a path; so is the reverse.
            Expr::BinOp(b) => {
                let left = self.resolve(&b.left);
                if left.as_deref() == Some("<path>") {
                    return left;
                }
                let right = self.resolve(&b.right);
                if right.as_deref() == Some("<path>") {
                    return right;
                }
                None
            }
            _ => None,
        }
    }

    /// A call to a name the file defines itself is the user's function, not a
    /// builtin of the same name.
    fn is_shadowed(&self, e: &Expr) -> bool {
        match e {
            Expr::Name(n) => {
                let id = n.id.as_str();
                self.locals.contains(id) && !self.aliases.contains_key(id)
            }
            _ => false,
        }
    }

    // --------------------------------------------------------------- effects

    #[allow(clippy::too_many_arguments)]
    fn push_hit(
        &mut self,
        effect: Effect,
        severity: Severity,
        verb: &str,
        symbol: &str,
        target: Option<String>,
        range: ruff_text_size::TextRange,
        note: Option<String>,
    ) {
        let snippet = clip(&squeeze(self.src.slice(range)), 72);
        self.effects.push(EffectHit {
            effect,
            severity,
            verb: verb.to_string(),
            symbol: symbol.to_string(),
            target,
            line: self.src.line_of(range.start()),
            col: self.src.col_of(range.start()),
            snippet,
            note,
        });
    }

    /// The literal text of a string expression, if it is one.
    fn str_lit(e: &Expr) -> Option<&str> {
        match e {
            Expr::StringLiteral(s) => Some(s.value.to_str()),
            _ => None,
        }
    }

    /// Render a call argument as the *thing being acted on*: a bare path, a
    /// URL, a joined command line — not a Python expression where we can help
    /// it, because the reader cares about the value, not the syntax.
    fn target_text(&self, e: &Expr) -> String {
        if let Some(s) = Self::str_lit(e) {
            return clip(&squeeze(s), TARGET);
        }
        // A name bound to a literal earlier in the file is far more useful
        // shown as its value than as its name.
        if let Expr::Name(n) = e {
            if let Some(value) = self.consts.get(n.id.as_str()) {
                return clip(&squeeze(value), TARGET);
            }
        }
        let elts: Option<&[Expr]> = match e {
            Expr::List(l) => Some(&l.elts),
            Expr::Tuple(t) => Some(&t.elts),
            _ => None,
        };
        if let Some(elts) = elts {
            if !elts.is_empty() && elts.iter().all(|x| Self::str_lit(x).is_some()) {
                let joined = elts
                    .iter()
                    .filter_map(Self::str_lit)
                    .collect::<Vec<_>>()
                    .join(" ");
                return clip(&squeeze(&joined), TARGET);
            }
        }
        brief_expr(self.src, e, TARGET)
    }

    fn extract_target(&self, call: &ExprCall, spec: TargetArg) -> Option<String> {
        let e = match spec {
            TargetArg::None => return None,
            TargetArg::Pos(i) => call.arguments.args.get(i)?,
            TargetArg::Named(name, i) => call.arguments.find_argument_value(name, i)?,
        };
        Some(self.target_text(e))
    }

    /// Scan a call's string arguments for shell fragments and paths that turn
    /// a routine operation into one worth reading closely.
    fn hazards(&self, call: &ExprCall) -> Vec<&'static str> {
        let mut found = Vec::new();
        let text = self.src.slice(call.range()).to_ascii_lowercase();
        for (needle, why) in effects::HAZARD_STRINGS {
            if text.contains(needle) && !found.contains(why) {
                found.push(*why);
            }
        }
        found
    }

    fn on_call(&mut self, call: &ExprCall) {
        self.metrics.calls += 1;
        if self.is_shadowed(&call.func) {
            return;
        }
        let Some(canonical) = self.resolve(&call.func) else {
            return;
        };
        let display = clip(&squeeze(self.src.slice(call.func.range())), 32);
        let line = self.src.line_of(call.range().start());

        // `open` is the one call whose effect depends on an argument, and it
        // is common enough to be worth special-casing.
        let (rule_path, forced) = if canonical == "open" || canonical == "io.open" {
            let mode = call
                .arguments
                .find_argument_value("mode", 1)
                .and_then(Self::str_lit)
                .unwrap_or("r");
            let (effect, sev, verb) = if mode.contains('w') {
                (Effect::FsWrite, Severity::Notable, "truncate")
            } else if mode.contains('a') {
                (Effect::FsWrite, Severity::Notable, "append")
            } else if mode.contains('x') {
                (Effect::FsWrite, Severity::Notable, "create")
            } else if mode.contains('+') {
                (Effect::FsWrite, Severity::Notable, "update")
            } else {
                (Effect::FsRead, Severity::Info, "read")
            };
            (canonical.clone(), Some((effect, sev, verb)))
        } else {
            (canonical.clone(), None)
        };

        let mut effect_mask = EffectMask::default();
        let hazards = self.hazards(call);

        if let Some((effect, sev, verb)) = forced {
            let target = self.extract_target(call, TargetArg::Named("file", 0));
            let mut sev = sev;
            let mut note = None;
            if !hazards.is_empty() {
                sev = sev.max(Severity::Caution);
                note = Some(hazards.join("; "));
            }
            effect_mask.insert(effect);
            self.push_hit(effect, sev, verb, &rule_path, target, call.range(), note);
        } else if let Some(rule) = effects::lookup(&canonical) {
            let target = self
                .extract_target(call, rule.target)
                .or_else(|| self.receiver_origin(&call.func));
            let mut sev = rule.sev;
            let mut notes: Vec<String> = rule.note.map(|n| n.to_string()).into_iter().collect();
            if !hazards.is_empty() {
                sev = sev.max(Severity::Caution);
                notes.extend(hazards.iter().map(|h| h.to_string()));
            }
            for kw in &call.arguments.keywords {
                let Some(name) = &kw.arg else { continue };
                let truthy = matches!(&kw.value, Expr::BooleanLiteral(b) if b.value);
                let falsy = matches!(&kw.value, Expr::BooleanLiteral(b) if !b.value);
                if truthy || falsy {
                    if let Some((msg, s)) = effects::kwarg_hazard(&canonical, name.as_str(), truthy)
                    {
                        notes.push(msg.to_string());
                        sev = sev.max(s);
                    }
                }
            }
            effect_mask.insert(rule.effect);
            let note = (!notes.is_empty()).then(|| notes.join("; "));
            self.push_hit(
                rule.effect,
                sev,
                rule.verb,
                &canonical,
                target,
                call.range(),
                note,
            );
        } else if let Some(rule) = canonical
            .rsplit_once('.')
            .filter(|(base, _)| !base.starts_with('<'))
            .and_then(|(_, method)| effects::method_lookup(method))
        {
            let target = self
                .extract_target(call, rule.target)
                .or_else(|| self.receiver_origin(&call.func));
            let mut sev = rule.sev;
            let mut notes: Vec<String> = rule.note.map(|n| n.to_string()).into_iter().collect();
            if !hazards.is_empty() {
                sev = sev.max(Severity::Caution);
                notes.extend(hazards.iter().map(|h| h.to_string()));
            }
            effect_mask.insert(rule.effect);
            let note = (!notes.is_empty()).then(|| notes.join("; "));
            self.push_hit(
                rule.effect,
                sev,
                rule.verb,
                &canonical,
                target,
                call.range(),
                note,
            );
        } else if effects::is_compute_module(&canonical) && canonical.contains('.') {
            effect_mask.insert(Effect::Compute);
            let root = canonical.split('.').next().unwrap_or("").to_string();
            self.push_hit(
                Effect::Compute,
                Severity::Info,
                "compute",
                &canonical,
                Some(root),
                call.range(),
                None,
            );
        }

        // Fold into the symbol inventory whether or not it had an effect: the
        // call list is useful on its own for seeing what a snippet leans on.
        let group = symbol_group(&canonical, &self.locals);
        let origin = self.symbol_origin(&canonical);
        let entry = self
            .symbols
            .entry(canonical.clone())
            .or_insert_with(|| SymbolUse {
                canonical: canonical.clone(),
                display: display.clone(),
                group,
                origin,
                count: 0,
                lines: Vec::new(),
                effects: EffectMask::default(),
            });
        entry.count += 1;
        if !entry.lines.contains(&line) {
            entry.lines.push(line);
        }
        entry.effects.union(effect_mask);
    }

    /// When a rule names no argument as its target, the receiver's own origin
    /// is usually the answer: `fh.write(x)` is about whatever `fh` was opened
    /// on.
    fn receiver_origin(&self, func: &Expr) -> Option<String> {
        let Expr::Attribute(attr) = func else {
            return None;
        };
        match &*attr.value {
            Expr::Name(name) => {
                if let Some(value) = self.consts.get(name.id.as_str()) {
                    return Some(clip(&squeeze(value), TARGET));
                }
                self.origins.get(name.id.as_str()).cloned()
            }
            // `(out / "flexible.json").write_text(...)` names its target
            // inline; show that rather than the bare method.
            other => Some(brief_expr(self.src, other, TARGET)),
        }
    }

    fn symbol_origin(&self, canonical: &str) -> SymbolOrigin {
        if canonical.starts_with('<') {
            return SymbolOrigin::Method;
        }
        match canonical.split_once('.') {
            None => {
                if self.locals.contains(canonical) {
                    SymbolOrigin::Local
                } else {
                    SymbolOrigin::Builtin
                }
            }
            Some((root, _)) => {
                if self.imported_roots.contains(root) || effects::is_stdlib(root) {
                    SymbolOrigin::Import
                } else {
                    SymbolOrigin::Method
                }
            }
        }
    }

    fn on_subscript(&mut self, sub: &ExprSubscript) {
        let Some(base) = self.resolve(&sub.value) else {
            return;
        };
        if base == "os.environ" {
            let target = Self::str_lit(&sub.slice).map(|s| s.to_string());
            self.push_hit(
                Effect::Env,
                Severity::Notable,
                "getenv",
                "os.environ",
                target,
                sub.range(),
                None,
            );
        }
    }

    fn on_attribute(&mut self, attr: &ExprAttribute) {
        let Some(path) = self.resolve(&attr.value) else {
            return;
        };
        let full = format!("{path}.{}", attr.attr);
        if full == "sys.argv" {
            self.push_hit(
                Effect::Env,
                Severity::Info,
                "argv",
                "sys.argv",
                None,
                attr.range(),
                None,
            );
        }
    }

    // ------------------------------------------------------------ expression

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Call(c) => {
                self.on_call(c);
                self.expr(&c.func);
                for a in c.arguments.args.iter() {
                    self.expr(a);
                }
                for k in &c.arguments.keywords {
                    self.expr(&k.value);
                }
            }
            Expr::Subscript(s) => {
                self.on_subscript(s);
                self.expr(&s.value);
                self.expr(&s.slice);
            }
            Expr::Attribute(a) => {
                self.on_attribute(a);
                self.expr(&a.value);
            }
            Expr::Name(n) => {
                if matches!(n.ctx, ExprContext::Load) {
                    *self.reads.entry(n.id.to_string()).or_default() += 1;
                }
            }
            Expr::BoolOp(b) => {
                self.metrics.complexity += b.values.len().saturating_sub(1) as u32;
                for v in &b.values {
                    self.expr(v);
                }
            }
            Expr::Named(n) => {
                if let Expr::Name(name) = &*n.target {
                    let kind = self.value_kind_of(&n.value);
                    self.bind(
                        name.id.as_str(),
                        BindKind::Walrus,
                        kind,
                        &n.value,
                        self.src.line_of(n.range().start()),
                    );
                }
                self.expr(&n.value);
            }
            Expr::BinOp(b) => {
                self.expr(&b.left);
                self.expr(&b.right);
            }
            Expr::UnaryOp(u) => self.expr(&u.operand),
            Expr::Lambda(l) => {
                if let Some(p) = &l.parameters {
                    for d in p.iter_non_variadic_params().filter_map(|p| p.default()) {
                        self.expr(d);
                    }
                }
                self.expr(&l.body);
            }
            Expr::If(i) => {
                self.metrics.complexity += 1;
                self.expr(&i.test);
                self.expr(&i.body);
                self.expr(&i.orelse);
            }
            Expr::Dict(d) => {
                for item in &d.items {
                    if let Some(k) = &item.key {
                        self.expr(k);
                    }
                    self.expr(&item.value);
                }
            }
            Expr::Set(s) => self.each(&s.elts),
            Expr::List(l) => self.each(&l.elts),
            Expr::Tuple(t) => self.each(&t.elts),
            Expr::ListComp(c) => {
                self.expr(&c.elt);
                self.comprehensions(&c.generators);
            }
            Expr::SetComp(c) => {
                self.expr(&c.elt);
                self.comprehensions(&c.generators);
            }
            Expr::Generator(c) => {
                self.expr(&c.elt);
                self.comprehensions(&c.generators);
            }
            Expr::DictComp(c) => {
                if let Some(k) = &c.key {
                    self.expr(k);
                }
                self.expr(&c.value);
                self.comprehensions(&c.generators);
            }
            Expr::Await(a) => self.expr(&a.value),
            Expr::Yield(y) => {
                if let Some(v) = &y.value {
                    self.expr(v);
                }
            }
            Expr::YieldFrom(y) => self.expr(&y.value),
            Expr::Compare(c) => {
                self.expr(&c.left);
                for x in c.comparators.iter() {
                    self.expr(x);
                }
            }
            Expr::Starred(s) => self.expr(&s.value),
            Expr::Slice(s) => {
                for part in [&s.lower, &s.upper, &s.step].into_iter().flatten() {
                    self.expr(part);
                }
            }
            Expr::FString(f) => {
                for element in f.value.elements().filter_map(|e| e.as_interpolation()) {
                    self.expr(&element.expression);
                }
            }
            Expr::TString(t) => {
                for element in t.value.elements().filter_map(|e| e.as_interpolation()) {
                    self.expr(&element.expression);
                }
            }
            _ => {}
        }
    }

    fn each(&mut self, list: &[Expr]) {
        for e in list {
            self.expr(e);
        }
    }

    fn comprehensions(&mut self, gens: &[ruff_python_ast::Comprehension]) {
        for g in gens {
            self.metrics.complexity += 1 + g.ifs.len() as u32;
            self.expr(&g.iter);
            for i in &g.ifs {
                self.expr(i);
            }
        }
    }

    // -------------------------------------------------------------- bindings

    fn value_kind_of(&self, e: &Expr) -> ValueKind {
        match e {
            Expr::StringLiteral(_) | Expr::FString(_) => return ValueKind::Text,
            Expr::NumberLiteral(_) => return ValueKind::Number,
            Expr::BooleanLiteral(_) => return ValueKind::Bool,
            Expr::NoneLiteral(_) => return ValueKind::None,
            Expr::List(_) | Expr::Tuple(_) | Expr::ListComp(_) | Expr::Set(_) => {
                return ValueKind::Seq
            }
            Expr::Dict(_) | Expr::DictComp(_) => return ValueKind::Map,
            Expr::Lambda(_) => return ValueKind::Callable,
            _ => {}
        }
        match self.resolve(e) {
            Some(path) => {
                let direct = effects::produces(&path);
                if direct != ValueKind::Unknown {
                    return direct;
                }
                // `f = open(p)` resolves to `<file>` already via `resolve`.
                match path.as_str() {
                    "<file>" => ValueKind::File,
                    "<path>" => ValueKind::Path,
                    "<session>" => ValueKind::Session,
                    "<response>" => ValueKind::Response,
                    "<frame>" => ValueKind::Frame,
                    "<proc>" => ValueKind::Process,
                    "<socket>" => ValueKind::Socket,
                    "<conn>" => ValueKind::Connection,
                    _ => ValueKind::Unknown,
                }
            }
            None => ValueKind::Unknown,
        }
    }

    fn bind(&mut self, name: &str, kind: BindKind, value: ValueKind, origin: &Expr, line: u32) {
        if name == "_" {
            return;
        }
        let origin_text = brief_expr(self.src, origin, ORIGIN);
        match Self::str_lit(origin) {
            Some(literal) if kind == BindKind::Assign => {
                self.consts.insert(name.to_string(), literal.to_string());
            }
            // A rebinding to something else invalidates the constant.
            _ => {
                self.consts.remove(name);
            }
        }
        self.origins.insert(name.to_string(), origin_text.clone());
        if value != ValueKind::Unknown {
            self.kinds.insert(name.to_string(), value);
        }
        self.locals.insert(name.to_string());
        if let Some(&idx) = self.binding_at.get(name) {
            let existing = &mut self.bindings[idx];
            existing.rebinds += 1;
            if kind == BindKind::Augment && self.loop_depth > 0 {
                existing.accumulates = true;
            }
            if existing.value == ValueKind::Unknown && value != ValueKind::Unknown {
                existing.value = value;
            }
            return;
        }
        self.binding_at
            .insert(name.to_string(), self.bindings.len());
        self.bindings.push(Binding {
            name: name.to_string(),
            kind,
            value,
            origin: origin_text,
            scope: self.scope_name(),
            line,
            depth: self.scope.len().saturating_sub(1) as u16,
            reads: 0,
            rebinds: 0,
            accumulates: kind == BindKind::Augment && self.loop_depth > 0,
            dead: false,
        });
    }

    fn bind_target(
        &mut self,
        target: &Expr,
        kind: BindKind,
        value: ValueKind,
        origin: &Expr,
        line: u32,
    ) {
        match target {
            Expr::Name(n) => self.bind(n.id.as_str(), kind, value, origin, line),
            Expr::Tuple(t) => {
                for e in &t.elts {
                    self.bind_target(e, kind, ValueKind::Unknown, origin, line);
                }
            }
            Expr::List(l) => {
                for e in &l.elts {
                    self.bind_target(e, kind, ValueKind::Unknown, origin, line);
                }
            }
            Expr::Starred(s) => self.bind_target(&s.value, kind, ValueKind::Seq, origin, line),
            _ => {}
        }
    }

    // ------------------------------------------------------------------ walk

    pub fn body(&mut self, stmts: &[Stmt], depth: u16) -> Vec<Node> {
        stmts.iter().map(|s| self.stmt(s, depth)).collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn finish(
        &mut self,
        id: u32,
        kind: NodeKind,
        label: String,
        detail: Option<String>,
        line: u32,
        end_line: u32,
        depth: u16,
        first_effect: usize,
        own_end: usize,
        children: Vec<Node>,
    ) -> Node {
        let mut own = EffectMask::default();
        let mut own_sev: Option<Severity> = None;
        let own_end = own_end.min(self.effects.len()).max(first_effect);
        for hit in &self.effects[first_effect..own_end] {
            own.insert(hit.effect);
            own_sev = Some(own_sev.map_or(hit.severity, |s: Severity| s.max(hit.severity)));
        }
        let mut all = own;
        let mut sev = own_sev;
        let mut weight = 1;
        for c in &children {
            all.union(c.effects);
            if let Some(cs) = c.severity {
                sev = Some(sev.map_or(cs, |s: Severity| s.max(cs)));
            }
            weight += c.weight;
        }
        self.max_depth = self.max_depth.max(depth);
        Node {
            id,
            kind,
            label,
            detail,
            line,
            end_line,
            depth,
            own_effects: own,
            effects: all,
            severity: sev,
            weight,
            children,
        }
    }

    fn stmt(&mut self, stmt: &Stmt, depth: u16) -> Node {
        self.metrics.statements += 1;
        let id = self.id();
        let line = stmt_line(self.src, stmt);
        let end_line = self.src.line_of(stmt.range().end());
        let e0 = self.effects.len();
        // Effects recorded between `e0` and `own_end` belong to this statement
        // itself; anything after that was raised by a nested statement and is
        // that statement's to report. Arms with children close the window
        // before they recurse; arms without children leave it open, and the
        // `min` in `finish` does the right thing.
        let mut own_end = usize::MAX;

        macro_rules! node {
            ($kind:expr, $label:expr, $detail:expr, $children:expr) => {
                self.finish(
                    id, $kind, $label, $detail, line, end_line, depth, e0, own_end, $children,
                )
            };
        }

        match stmt {
            Stmt::Import(s) => {
                self.metrics.imports += s.names.len() as u32;
                let names: Vec<String> = s
                    .names
                    .iter()
                    .map(|a| match &a.asname {
                        Some(x) => format!("{} as {x}", a.name),
                        None => a.name.to_string(),
                    })
                    .collect();
                node!(
                    NodeKind::Import,
                    format!("import {}", names.join(", ")),
                    None,
                    vec![]
                )
            }
            Stmt::ImportFrom(s) => {
                self.metrics.imports += s.names.len() as u32;
                let module = match &s.module {
                    Some(m) => format!("{}{m}", ".".repeat(s.level as usize)),
                    None => ".".repeat(s.level.max(1) as usize),
                };
                let names: Vec<String> = s
                    .names
                    .iter()
                    .map(|a| match &a.asname {
                        Some(x) => format!("{} as {x}", a.name),
                        None => a.name.to_string(),
                    })
                    .collect();
                let label = clip(&format!("from {module} import {}", names.join(", ")), LABEL);
                node!(NodeKind::Import, label, None, vec![])
            }
            Stmt::FunctionDef(s) => {
                self.metrics.functions += 1;
                if s.is_async {
                    self.metrics.async_defs += 1;
                }
                for d in &s.decorator_list {
                    self.expr(&d.expression);
                }
                for p in s
                    .parameters
                    .iter_non_variadic_params()
                    .filter_map(|p| p.default())
                {
                    self.expr(p);
                }
                let kw = if s.is_async { "async def" } else { "def" };
                let label = clip(
                    &format!("{kw} {}({})", s.name, param_names(&s.parameters).join(", ")),
                    LABEL,
                );
                let mut detail = Vec::new();
                if let Some(ret) = &s.returns {
                    detail.push(format!("→ {}", brief_expr(self.src, ret, 24)));
                }
                for d in &s.decorator_list {
                    detail.push(format!("@{}", brief_expr(self.src, &d.expression, 20)));
                }
                own_end = self.effects.len();
                self.scope.push(s.name.to_string());
                let saved_loop = std::mem::replace(&mut self.loop_depth, 0);
                let children = self.body(&s.body, depth + 1);
                self.loop_depth = saved_loop;
                self.scope.pop();
                self.bind_name_only(
                    s.name.as_str(),
                    BindKind::Def,
                    ValueKind::Callable,
                    &label,
                    line,
                );
                let detail = (!detail.is_empty()).then(|| detail.join("  "));
                node!(NodeKind::Def, label, detail, children)
            }
            Stmt::ClassDef(s) => {
                self.metrics.classes += 1;
                for d in &s.decorator_list {
                    self.expr(&d.expression);
                }
                let bases = s
                    .arguments
                    .as_ref()
                    .map(|a| {
                        a.args
                            .iter()
                            .map(|e| brief_expr(self.src, e, 20))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let label = if bases.is_empty() {
                    format!("class {}", s.name)
                } else {
                    clip(&format!("class {}({bases})", s.name), LABEL)
                };
                own_end = self.effects.len();
                self.scope.push(s.name.to_string());
                let children = self.body(&s.body, depth + 1);
                self.scope.pop();
                self.bind_name_only(
                    s.name.as_str(),
                    BindKind::Class,
                    ValueKind::Class,
                    &label,
                    line,
                );
                node!(NodeKind::Class, label, None, children)
            }
            Stmt::For(s) => {
                self.metrics.loops += 1;
                self.metrics.complexity += 1;
                self.expr(&s.iter);
                let value = self.value_kind_of(&s.iter);
                let element = match value {
                    ValueKind::Seq | ValueKind::Map => ValueKind::Unknown,
                    other => other,
                };
                self.bind_target(&s.target, BindKind::LoopVar, element, &s.iter, line);
                let kw = if s.is_async { "async for" } else { "for" };
                let label = clip(
                    &format!(
                        "{kw} {} in {}",
                        brief_expr(self.src, &s.target, 24),
                        brief_expr(self.src, &s.iter, LABEL / 2)
                    ),
                    LABEL,
                );
                own_end = self.effects.len();
                self.loop_depth += 1;
                let mut children = self.body(&s.body, depth + 1);
                self.loop_depth -= 1;
                if !s.orelse.is_empty() {
                    children.push(self.arm("else", &s.orelse, depth + 1));
                }
                node!(NodeKind::Loop, label, None, children)
            }
            Stmt::While(s) => {
                self.metrics.loops += 1;
                self.metrics.complexity += 1;
                self.expr(&s.test);
                let label = clip(
                    &format!("while {}", brief_expr(self.src, &s.test, LABEL - 6)),
                    LABEL,
                );
                own_end = self.effects.len();
                self.loop_depth += 1;
                let mut children = self.body(&s.body, depth + 1);
                self.loop_depth -= 1;
                if !s.orelse.is_empty() {
                    children.push(self.arm("else", &s.orelse, depth + 1));
                }
                node!(NodeKind::Loop, label, None, children)
            }
            Stmt::If(s) => {
                self.metrics.branches += 1;
                self.metrics.complexity += 1;
                self.expr(&s.test);
                let label = clip(
                    &format!("if {}", brief_expr(self.src, &s.test, LABEL - 3)),
                    LABEL,
                );
                own_end = self.effects.len();
                let mut children = self.body(&s.body, depth + 1);
                for clause in &s.elif_else_clauses {
                    let arm_id = self.id();
                    let arm_line = self.src.line_of(clause.range().start());
                    let arm_end = self.src.line_of(clause.range().end());
                    let a0 = self.effects.len();
                    let arm_label = match &clause.test {
                        Some(test) => {
                            self.metrics.branches += 1;
                            self.metrics.complexity += 1;
                            self.expr(test);
                            clip(
                                &format!("elif {}", brief_expr(self.src, test, LABEL - 5)),
                                LABEL,
                            )
                        }
                        None => "else".to_string(),
                    };
                    let arm_own = self.effects.len();
                    let arm_children = self.body(&clause.body, depth + 2);
                    children.push(self.finish(
                        arm_id,
                        NodeKind::BranchArm,
                        arm_label,
                        None,
                        arm_line,
                        arm_end,
                        depth + 1,
                        a0,
                        arm_own,
                        arm_children,
                    ));
                }
                node!(NodeKind::Branch, label, None, children)
            }
            Stmt::Match(s) => {
                self.metrics.branches += 1;
                self.expr(&s.subject);
                let label = clip(
                    &format!("match {}", brief_expr(self.src, &s.subject, LABEL - 6)),
                    LABEL,
                );
                own_end = self.effects.len();
                let mut children = Vec::new();
                for case in &s.cases {
                    self.metrics.complexity += 1;
                    let arm_id = self.id();
                    let a0 = self.effects.len();
                    let pat = clip(&squeeze(self.src.slice(case.pattern.range())), LABEL - 6);
                    let arm_own = self.effects.len();
                    let arm_children = self.body(&case.body, depth + 2);
                    children.push(self.finish(
                        arm_id,
                        NodeKind::BranchArm,
                        format!("case {pat}"),
                        None,
                        self.src.line_of(case.range().start()),
                        self.src.line_of(case.range().end()),
                        depth + 1,
                        a0,
                        arm_own,
                        arm_children,
                    ));
                }
                node!(NodeKind::Branch, label, None, children)
            }
            Stmt::With(s) => {
                for item in &s.items {
                    self.expr(&item.context_expr);
                    if let Some(vars) = &item.optional_vars {
                        let kind = self.value_kind_of(&item.context_expr);
                        self.bind_target(vars, BindKind::WithVar, kind, &item.context_expr, line);
                    }
                }
                let items: Vec<String> = s
                    .items
                    .iter()
                    .map(|item| {
                        let ctx = brief_expr(self.src, &item.context_expr, 34);
                        match &item.optional_vars {
                            Some(v) => format!("{ctx} as {}", brief_expr(self.src, v, 12)),
                            None => ctx,
                        }
                    })
                    .collect();
                let kw = if s.is_async { "async with" } else { "with" };
                let label = clip(&format!("{kw} {}", items.join(", ")), LABEL);
                own_end = self.effects.len();
                let children = self.body(&s.body, depth + 1);
                node!(NodeKind::With, label, None, children)
            }
            Stmt::Try(s) => {
                own_end = e0;
                let mut children = self.body(&s.body, depth + 1);
                for handler in &s.handlers {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    self.metrics.handlers += 1;
                    self.metrics.complexity += 1;
                    let hid = self.id();
                    let h0 = self.effects.len();
                    if let Some(t) = &h.type_ {
                        self.expr(t);
                    }
                    let mut label = String::from(if s.is_star { "except*" } else { "except" });
                    if let Some(t) = &h.type_ {
                        label.push(' ');
                        label.push_str(&brief_expr(self.src, t, 28));
                    }
                    if let Some(name) = &h.name {
                        label.push_str(&format!(" as {name}"));
                    }
                    let h_own = self.effects.len();
                    let hchildren = self.body(&h.body, depth + 2);
                    children.push(self.finish(
                        hid,
                        NodeKind::Handler,
                        label,
                        None,
                        self.src.line_of(h.range().start()),
                        self.src.line_of(h.range().end()),
                        depth + 1,
                        h0,
                        h_own,
                        hchildren,
                    ));
                }
                if !s.orelse.is_empty() {
                    children.push(self.arm("else", &s.orelse, depth + 1));
                }
                if !s.finalbody.is_empty() {
                    children.push(self.arm("finally", &s.finalbody, depth + 1));
                }
                node!(NodeKind::Try, "try".into(), None, children)
            }
            Stmt::Assign(s) => {
                self.expr(&s.value);
                let kind = self.value_kind_of(&s.value);
                for t in &s.targets {
                    self.expr_store(t);
                    self.bind_target(t, BindKind::Assign, kind, &s.value, line);
                }
                let lhs = s
                    .targets
                    .iter()
                    .map(|t| brief_expr(self.src, t, 24))
                    .collect::<Vec<_>>()
                    .join(" = ");
                let rhs = brief_expr(self.src, &s.value, LABEL.saturating_sub(lhs.len() + 3));
                node!(
                    NodeKind::Assign,
                    clip(&format!("{lhs} = {rhs}"), LABEL),
                    None,
                    vec![]
                )
            }
            Stmt::AugAssign(s) => {
                self.expr(&s.value);
                self.expr_store(&s.target);
                self.bind_target(
                    &s.target,
                    BindKind::Augment,
                    ValueKind::Unknown,
                    &s.value,
                    line,
                );
                let lhs = brief_expr(self.src, &s.target, 24);
                let rhs = brief_expr(self.src, &s.value, LABEL.saturating_sub(lhs.len() + 4));
                node!(
                    NodeKind::Assign,
                    clip(&format!("{lhs} {}= {rhs}", s.op.as_str()), LABEL),
                    None,
                    vec![]
                )
            }
            Stmt::AnnAssign(s) => {
                if let Some(v) = &s.value {
                    self.expr(v);
                    let kind = self.value_kind_of(v);
                    self.bind_target(&s.target, BindKind::Annotate, kind, v, line);
                }
                let label = clip(&squeeze(self.src.slice(s.range())), LABEL);
                node!(NodeKind::Assign, label, None, vec![])
            }
            Stmt::Return(s) => {
                if let Some(v) = &s.value {
                    self.expr(v);
                }
                let label = match &s.value {
                    Some(v) => clip(
                        &format!("return {}", brief_expr(self.src, v, LABEL - 7)),
                        LABEL,
                    ),
                    None => "return".into(),
                };
                node!(NodeKind::Return, label, None, vec![])
            }
            Stmt::Raise(s) => {
                for e in [&s.exc, &s.cause].into_iter().flatten() {
                    self.expr(e);
                }
                let label = match &s.exc {
                    Some(e) => clip(
                        &format!("raise {}", brief_expr(self.src, e, LABEL - 6)),
                        LABEL,
                    ),
                    None => "raise".into(),
                };
                node!(NodeKind::Raise, label, None, vec![])
            }
            Stmt::Assert(s) => {
                self.expr(&s.test);
                if let Some(m) = &s.msg {
                    self.expr(m);
                }
                self.metrics.complexity += 1;
                let label = clip(
                    &format!("assert {}", brief_expr(self.src, &s.test, LABEL - 7)),
                    LABEL,
                );
                node!(NodeKind::Assert, label, None, vec![])
            }
            Stmt::Delete(s) => {
                self.each(&s.targets);
                let names = s
                    .targets
                    .iter()
                    .map(|t| brief_expr(self.src, t, 20))
                    .collect::<Vec<_>>()
                    .join(", ");
                node!(
                    NodeKind::Delete,
                    clip(&format!("del {names}"), LABEL),
                    None,
                    vec![]
                )
            }
            Stmt::Global(s) => {
                let names = s
                    .names
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                node!(NodeKind::Global, format!("global {names}"), None, vec![])
            }
            Stmt::Nonlocal(s) => {
                let names = s
                    .names
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                node!(NodeKind::Global, format!("nonlocal {names}"), None, vec![])
            }
            Stmt::Expr(s) => {
                self.expr(&s.value);
                let (kind, label, detail) = match &*s.value {
                    Expr::StringLiteral(lit) => {
                        let text = squeeze(lit.value.to_str());
                        (
                            NodeKind::Expr,
                            "\"\"\"docstring\"\"\"".to_string(),
                            Some(clip(&text, LABEL)),
                        )
                    }
                    Expr::Call(_) => (
                        NodeKind::Call,
                        clip(&brief_expr(self.src, &s.value, LABEL), LABEL),
                        None,
                    ),
                    _ => (
                        NodeKind::Expr,
                        clip(&brief_expr(self.src, &s.value, LABEL), LABEL),
                        None,
                    ),
                };
                node!(kind, label, detail, vec![])
            }
            Stmt::Pass(_) => node!(NodeKind::Pass, "pass".into(), None, vec![]),
            Stmt::Break(_) => node!(NodeKind::Jump, "break".into(), None, vec![]),
            Stmt::Continue(_) => node!(NodeKind::Jump, "continue".into(), None, vec![]),
            Stmt::TypeAlias(s) => {
                let label = clip(&squeeze(self.src.slice(s.range())), LABEL);
                node!(NodeKind::Assign, label, None, vec![])
            }
            Stmt::IpyEscapeCommand(s) => {
                let label = clip(&squeeze(self.src.slice(s.range())), LABEL);
                node!(NodeKind::Call, label, None, vec![])
            }
        }
    }

    /// Walk a store target for its *nested* loads — `d[k] = v` reads `d` and
    /// `k` even though the statement as a whole is a write.
    fn expr_store(&mut self, target: &Expr) {
        match target {
            Expr::Subscript(s) => {
                self.expr(&s.value);
                self.expr(&s.slice);
            }
            Expr::Attribute(a) => self.expr(&a.value),
            Expr::Tuple(t) => {
                for e in &t.elts {
                    self.expr_store(e);
                }
            }
            Expr::List(l) => {
                for e in &l.elts {
                    self.expr_store(e);
                }
            }
            Expr::Starred(s) => self.expr_store(&s.value),
            _ => {}
        }
    }

    fn arm(&mut self, keyword: &str, body: &[Stmt], depth: u16) -> Node {
        let id = self.id();
        let e0 = self.effects.len();
        let line = body.first().map(|s| stmt_line(self.src, s)).unwrap_or(0);
        let end = body
            .last()
            .map(|s| self.src.line_of(s.range().end()))
            .unwrap_or(line);
        let children = self.body(body, depth + 1);
        self.finish(
            id,
            NodeKind::BranchArm,
            keyword.to_string(),
            None,
            line,
            end,
            depth,
            e0,
            e0,
            children,
        )
    }

    /// Record a binding whose origin is a label we already built (a `def` or
    /// `class` header) rather than an expression.
    fn bind_name_only(
        &mut self,
        name: &str,
        kind: BindKind,
        value: ValueKind,
        label: &str,
        line: u32,
    ) {
        self.kinds.insert(name.to_string(), value);
        self.locals.insert(name.to_string());
        if self.binding_at.contains_key(name) {
            return;
        }
        self.binding_at
            .insert(name.to_string(), self.bindings.len());
        self.bindings.push(Binding {
            name: name.to_string(),
            kind,
            value,
            origin: clip(label, ORIGIN),
            scope: self.scope_name(),
            line,
            depth: self.scope.len().saturating_sub(1) as u16,
            reads: 0,
            rebinds: 0,
            accumulates: false,
            dead: false,
        });
    }

    pub fn into_parts(self) -> Findings {
        let mut bindings = self.bindings;
        bindings.sort_by_key(|b| (b.line, b.name.clone()));
        let mut symbols: Vec<SymbolUse> = self.symbols.into_values().collect();
        symbols.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then(a.group.cmp(&b.group))
                .then(a.canonical.cmp(&b.canonical))
        });
        Findings {
            imports: self.imports,
            effects: self.effects,
            symbols,
            bindings,
            diagnostics: self.diagnostics,
            metrics: self.metrics,
            reads: self.reads,
            max_depth: self.max_depth,
        }
    }
}

/// Everything the walk collected, handed over in one piece.
pub struct Findings {
    pub imports: Vec<ImportInfo>,
    pub effects: Vec<EffectHit>,
    pub symbols: Vec<SymbolUse>,
    pub bindings: Vec<Binding>,
    pub diagnostics: Vec<Diagnostic>,
    pub metrics: Metrics,
    /// Name → how many times it was read, for spotting dead bindings.
    pub reads: HashMap<String, u32>,
    pub max_depth: u16,
}

fn symbol_group(canonical: &str, locals: &HashSet<String>) -> String {
    if canonical.starts_with('<') {
        return canonical
            .split('.')
            .next()
            .unwrap_or(canonical)
            .trim_matches(|c| c == '<' || c == '>')
            .to_string();
    }
    match canonical.rsplit_once('.') {
        Some((module, _)) => module.to_string(),
        None => {
            if locals.contains(canonical) {
                "<local>".into()
            } else {
                "<builtins>".into()
            }
        }
    }
}

/// Names that would otherwise dominate the call inventory without telling the
/// reader anything.
pub fn is_noise_symbol(canonical: &str) -> bool {
    matches!(
        canonical,
        "len"
            | "str"
            | "int"
            | "float"
            | "list"
            | "dict"
            | "set"
            | "tuple"
            | "range"
            | "enumerate"
            | "zip"
            | "sorted"
            | "isinstance"
            | "type"
            | "bool"
            | "abs"
            | "min"
            | "max"
            | "sum"
            | "repr"
            | "format"
            | "getattr"
            | "hasattr"
            | "super"
    )
}

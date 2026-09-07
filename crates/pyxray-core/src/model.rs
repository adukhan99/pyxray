//! The X-ray report: everything `pyxray` knows about a snippet, with no
//! opinion at all about how it should look. `pyxray-render` turns this into
//! pixels; the JSON form is the stable contract for anything else.

use serde::{Deserialize, Serialize};

/// A capability the snippet reaches for. Deliberately a short, memorable set —
/// every variant gets its own colour and glyph downstream, and a list longer
/// than a dozen or so stops being glanceable.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    FsRead,
    FsWrite,
    FsDelete,
    Net,
    Process,
    Env,
    Dynamic,
    Stdout,
    Random,
    Clock,
    Concurrency,
    Compute,
    Exit,
}

impl Effect {
    pub const ALL: [Effect; 13] = [
        Effect::FsRead,
        Effect::FsWrite,
        Effect::FsDelete,
        Effect::Net,
        Effect::Process,
        Effect::Env,
        Effect::Dynamic,
        Effect::Stdout,
        Effect::Random,
        Effect::Clock,
        Effect::Concurrency,
        Effect::Compute,
        Effect::Exit,
    ];

    /// Stable short id, used in JSON, CLI filters and theme lookup tables.
    pub fn id(self) -> &'static str {
        match self {
            Effect::FsRead => "fs.read",
            Effect::FsWrite => "fs.write",
            Effect::FsDelete => "fs.delete",
            Effect::Net => "net",
            Effect::Process => "process",
            Effect::Env => "env",
            Effect::Dynamic => "dynamic",
            Effect::Stdout => "stdout",
            Effect::Random => "random",
            Effect::Clock => "clock",
            Effect::Concurrency => "concurrency",
            Effect::Compute => "compute",
            Effect::Exit => "exit",
        }
    }

    /// Human label for panel headings.
    pub fn label(self) -> &'static str {
        match self {
            Effect::FsRead => "reads files",
            Effect::FsWrite => "writes files",
            Effect::FsDelete => "deletes files",
            Effect::Net => "network",
            Effect::Process => "runs commands",
            Effect::Env => "environment",
            Effect::Dynamic => "dynamic code",
            Effect::Stdout => "prints",
            Effect::Random => "randomness",
            Effect::Clock => "time",
            Effect::Concurrency => "concurrency",
            Effect::Compute => "compute",
            Effect::Exit => "exits",
        }
    }

    pub fn bit(self) -> u16 {
        1u16 << (self as u16)
    }

    pub fn from_id(s: &str) -> Option<Effect> {
        Effect::ALL.into_iter().find(|e| e.id() == s)
    }
}

/// Union of effects over a subtree. Cheap enough to hang off every spine node.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectMask(pub u16);

impl EffectMask {
    pub fn insert(&mut self, e: Effect) {
        self.0 |= e.bit();
    }
    pub fn union(&mut self, other: EffectMask) {
        self.0 |= other.0;
    }
    pub fn contains(self, e: Effect) -> bool {
        self.0 & e.bit() != 0
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn iter(self) -> impl Iterator<Item = Effect> {
        Effect::ALL.into_iter().filter(move |e| self.contains(*e))
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Ordinary and expected — printing, importing, arithmetic.
    Info,
    /// Worth a glance before you run it — writes, network, environment reads.
    Notable,
    /// Stop and read the code — deletion, shells, eval, pickle.
    Caution,
}

/// One concrete place the snippet exercises a capability.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EffectHit {
    pub effect: Effect,
    pub severity: Severity,
    /// Imperative verb for the timeline: "read", "write", "spawn".
    pub verb: String,
    /// Resolved dotted callee, e.g. `subprocess.run`, `<file>.write`.
    pub symbol: String,
    /// The thing acted upon, when we can name it: a path, URL, command.
    pub target: Option<String>,
    pub line: u32,
    pub col: u32,
    /// Normalised source of the call itself.
    pub snippet: String,
    /// Why this was flagged, when the reason is not obvious from the symbol.
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Module,
    Import,
    Def,
    Class,
    Loop,
    Branch,
    BranchArm,
    Try,
    Handler,
    With,
    Assign,
    Call,
    Return,
    Raise,
    Assert,
    Jump,
    Expr,
    Pass,
    Global,
    Delete,
}

impl NodeKind {
    pub fn id(self) -> &'static str {
        match self {
            NodeKind::Module => "module",
            NodeKind::Import => "import",
            NodeKind::Def => "def",
            NodeKind::Class => "class",
            NodeKind::Loop => "loop",
            NodeKind::Branch => "branch",
            NodeKind::BranchArm => "arm",
            NodeKind::Try => "try",
            NodeKind::Handler => "handler",
            NodeKind::With => "with",
            NodeKind::Assign => "assign",
            NodeKind::Call => "call",
            NodeKind::Return => "return",
            NodeKind::Raise => "raise",
            NodeKind::Assert => "assert",
            NodeKind::Jump => "jump",
            NodeKind::Expr => "expr",
            NodeKind::Pass => "pass",
            NodeKind::Global => "global",
            NodeKind::Delete => "delete",
        }
    }

    /// Does this kind introduce a nesting level a reader cares about?
    pub fn is_block(self) -> bool {
        matches!(
            self,
            NodeKind::Def
                | NodeKind::Class
                | NodeKind::Loop
                | NodeKind::Branch
                | NodeKind::BranchArm
                | NodeKind::Try
                | NodeKind::Handler
                | NodeKind::With
                | NodeKind::Module
        )
    }
}

/// One entry in the structural outline. Labels are already human-readable —
/// the renderer never needs to know Python.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Node {
    pub id: u32,
    pub kind: NodeKind,
    /// e.g. `for path in paths`, `def load(path, *, strict=False)`.
    pub label: String,
    /// Secondary text: a return type, a decorator list, an effect target.
    pub detail: Option<String>,
    pub line: u32,
    pub end_line: u32,
    pub depth: u16,
    /// Effects raised directly by this statement.
    pub own_effects: EffectMask,
    /// Effects raised anywhere in this subtree.
    pub effects: EffectMask,
    /// Highest severity anywhere in this subtree.
    pub severity: Option<Severity>,
    /// Statements in this subtree, including this one.
    pub weight: u32,
    pub children: Vec<Node>,
}

impl Node {
    pub fn walk(&self, f: &mut impl FnMut(&Node)) {
        f(self);
        for c in &self.children {
            c.walk(f);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImportInfo {
    /// Canonical module path, e.g. `numpy`, `os.path`, `.relative`.
    pub module: String,
    /// Names pulled out of it by `from x import a, b`; empty for plain imports.
    pub names: Vec<String>,
    /// Local alias the module is bound to, when renamed.
    pub alias: Option<String>,
    pub line: u32,
    /// Not in the standard library — i.e. something that has to be installed.
    pub third_party: bool,
    /// Inside a function/try, rather than at module top level.
    pub deferred: bool,
}

/// How a name came to exist, and what it appears to hold.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BindKind {
    Assign,
    Augment,
    Annotate,
    LoopVar,
    WithVar,
    Param,
    Walrus,
    Comprehension,
    ExceptVar,
    Import,
    Def,
    Class,
}

/// A coarse guess at what a name holds, used to resolve method calls
/// (`f.write(...)` after `f = open(...)`) and to colour the bindings panel.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Unknown,
    File,
    Path,
    Response,
    Session,
    Process,
    Socket,
    Connection,
    Frame,
    Array,
    Tensor,
    Text,
    Number,
    Bool,
    Seq,
    Map,
    Callable,
    Module,
    Class,
    None,
}

impl ValueKind {
    pub fn id(self) -> &'static str {
        match self {
            ValueKind::Unknown => "?",
            ValueKind::File => "file",
            ValueKind::Path => "path",
            ValueKind::Response => "response",
            ValueKind::Session => "session",
            ValueKind::Process => "proc",
            ValueKind::Socket => "socket",
            ValueKind::Connection => "conn",
            ValueKind::Frame => "frame",
            ValueKind::Array => "array",
            ValueKind::Tensor => "tensor",
            ValueKind::Text => "str",
            ValueKind::Number => "num",
            ValueKind::Bool => "bool",
            ValueKind::Seq => "seq",
            ValueKind::Map => "map",
            ValueKind::Callable => "fn",
            ValueKind::Module => "mod",
            ValueKind::Class => "class",
            ValueKind::None => "none",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Binding {
    pub name: String,
    pub kind: BindKind,
    pub value: ValueKind,
    /// Normalised source of what it was bound to.
    pub origin: String,
    /// Enclosing scope path, e.g. `<module>` or `Loader.read`.
    pub scope: String,
    pub line: u32,
    pub depth: u16,
    /// Number of later reads of this name.
    pub reads: u32,
    /// Rebound or augmented after first binding.
    pub rebinds: u32,
    /// Augmented inside a loop — i.e. it accumulates.
    pub accumulates: bool,
    /// Bound but never read afterwards.
    pub dead: bool,
}

/// Where a called symbol comes from, which decides whether it is worth listing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolOrigin {
    /// Reached through an import in this file.
    Import,
    /// A Python builtin.
    Builtin,
    /// Defined in this file.
    Local,
    /// A method on a value whose type we could not determine.
    Method,
}

/// An externally-defined thing the snippet calls, folded across call sites.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SymbolUse {
    /// Resolved dotted path, e.g. `numpy.array`.
    pub canonical: String,
    /// As written, e.g. `np.array`.
    pub display: String,
    /// Owning module for grouping; `<builtins>` or `<local>` when there is none.
    pub group: String,
    pub origin: SymbolOrigin,
    pub count: u32,
    pub lines: Vec<u32>,
    pub effects: EffectMask,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct Metrics {
    pub lines_total: u32,
    pub lines_code: u32,
    pub lines_comment: u32,
    pub lines_blank: u32,
    pub statements: u32,
    pub max_depth: u32,
    /// Cyclomatic-ish: 1 + every branch, loop, handler and boolean operator.
    pub complexity: u32,
    pub functions: u32,
    pub classes: u32,
    pub async_defs: u32,
    pub calls: u32,
    pub imports: u32,
    pub loops: u32,
    pub branches: u32,
    pub handlers: u32,
    /// 0..100, from the severity and spread of the effects found.
    pub risk: u8,
}

/// What one source line looks like, for the minimap strip.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct LineCell {
    pub indent: u8,
    pub width: u16,
    pub blank: bool,
    pub comment: bool,
    pub kind: Option<NodeKind>,
    pub effects: EffectMask,
    pub severity: Option<Severity>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagLevel {
    Error,
    Warning,
    Hint,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Diagnostic {
    pub level: DiagLevel,
    pub message: String,
    pub line: u32,
    pub col: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Meta {
    /// Display name for the snippet: a filename, or `<stdin>`.
    pub name: String,
    pub bytes: usize,
    /// True when the source parsed cleanly; a failed parse still yields a
    /// partial report from whatever the recovering parser produced.
    pub parsed: bool,
    /// One-line plain-English answer to "what does this do?".
    pub synopsis: String,
    /// Ranked verbs behind the synopsis, for renderers that want the pieces.
    pub headline: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Report {
    pub meta: Meta,
    pub spine: Node,
    pub imports: Vec<ImportInfo>,
    pub effects: Vec<EffectHit>,
    pub symbols: Vec<SymbolUse>,
    pub bindings: Vec<Binding>,
    pub metrics: Metrics,
    pub texture: Vec<LineCell>,
    pub diagnostics: Vec<Diagnostic>,
    /// The source, kept so renderers can show code beside the analysis.
    pub source: Vec<String>,
}

impl Report {
    /// Effects folded by kind, most severe first then most frequent, for the
    /// capability panel.
    pub fn effect_summary(&self) -> Vec<(Effect, Severity, u32)> {
        let mut out: Vec<(Effect, Severity, u32)> = Vec::new();
        for hit in &self.effects {
            match out.iter_mut().find(|(e, _, _)| *e == hit.effect) {
                Some(slot) => {
                    slot.1 = slot.1.max(hit.severity);
                    slot.2 += 1;
                }
                None => out.push((hit.effect, hit.severity, 1)),
            }
        }
        // Cautions lead; everything else falls into the order the program
        // runs in, so the row reads as a pipeline.
        out.sort_by(|a, b| {
            let rank = |s: Severity| u8::from(s != Severity::Caution);
            rank(a.1)
                .cmp(&rank(b.1))
                .then(crate::digest::stage(a.0).cmp(&crate::digest::stage(b.0)))
                .then(b.2.cmp(&a.2))
                .then(a.0.cmp(&b.0))
        });
        out
    }

    /// The effect hits worth showing when there is only room for `limit` rows:
    /// identical operations folded together, the most severe kept, and the
    /// survivors put back into source order.
    pub fn timeline(&self, limit: usize) -> Vec<(&EffectHit, u32)> {
        let mut folded: Vec<(&EffectHit, u32)> = Vec::new();
        for hit in &self.effects {
            match folded.iter_mut().find(|(h, _)| {
                h.effect == hit.effect && h.verb == hit.verb && h.target == hit.target
            }) {
                Some(slot) => slot.1 += 1,
                None => folded.push((hit, 1)),
            }
        }
        if folded.len() > limit {
            folded.sort_by(|a, b| {
                b.0.severity
                    .cmp(&a.0.severity)
                    .then(b.1.cmp(&a.1))
                    .then(a.0.line.cmp(&b.0.line))
            });
            folded.truncate(limit);
        }
        folded.sort_by_key(|(h, _)| (h.line, h.col));
        folded
    }

    pub fn mask(&self) -> EffectMask {
        self.spine.effects
    }

    pub fn worst(&self) -> Option<Severity> {
        self.effects.iter().map(|h| h.severity).max()
    }
}

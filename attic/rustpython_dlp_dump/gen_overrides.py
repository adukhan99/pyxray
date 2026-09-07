#!/usr/bin/env python3
"""Generate src/overrides.rs: one `visit_*` override per dispatchable AST node.

Each override emits the node line (via DumpVisitor::emit) and then *manually*
walks every AST child with correct `self.indent` management, because several
`generic_visit_*` walkers are empty no-op stubs in rustpython-parser's
`Visitor` trait and would otherwise silently drop children (e.g. `arguments`,
`withitem`, `match_case`, `keyword`, `comprehension`).

Types that have no `visit_*` method of their own (`ArgWithDefault`,
`PythonArguments`) are inlined at their use site instead of recursed into.
"""
import json
import re

DATA = json.load(open("ast_data.json"))
STRUCTS = DATA["structs"]  # Name -> [[field, type], ...]

# Dispatchable node types (have a `visit_*` method in the Visitor trait).
DISPATCH = {}   # Type -> visit_method
for line in open("src/visitor.rs"):
    m = re.search(r"fn (visit_[a-z0-9_]+)\(&mut self, node: ([A-Za-z0-9_]+)<R>", line)
    if m:
        DISPATCH[m.group(2)] = m.group(1)

# Types that are AST nodes but have NO visit_* method (composites inlined).
NO_METHOD = {"ArgWithDefault", "PythonArguments"}

# Node types whose Rust struct is generic over R (need a concrete type
# annotation so `Xxx::NAME` can resolve `NAME`).
GENERIC = {n for n, fields in STRUCTS.items()
            if any(t.strip() == "R" or "<R>" in t for f, t in fields)}


def label(name):
    """emit() label expression: `Xxx::NAME` or `Xxx::<TextRange>::NAME`."""
    return f"{name}::<TextRange>::NAME" if name in GENERIC else f"{name}::NAME"



def strip(t):
    """Bare contained AST-node type name for a field type, or None."""
    t = t.strip()
    m = re.match(r"^(?:Vec|Option)<(.*)>$", t)
    if m:
        return strip(m.group(1))
    if t.startswith("Box<"):
        return strip(t[4:-1])
    if t.endswith("<R>"):
        t = t[:-3]
    t = t.strip()
    # Recognize BOTH struct node types and dispatchable enum node types
    # (Stmt, Expr, Pattern, ExceptHandler, TypeParam).  Enum-typed fields were
    # previously dropped entirely because `t in STRUCTS` was False for them,
    # which made collect() fall through to leaf_renders() and emit nothing.
    return t if (t in STRUCTS or t in DISPATCH) else None


def indent_for(rec):
    return "            " if rec == 0 else "                "


def field_walk(field, typ, rec, access="node"):
    """Walk statement for a node-typed field, or inlined body if NO_METHOD."""
    ind = indent_for(rec)
    t = typ.strip()
    contained = strip(t)
    if contained in DISPATCH:
        inner = contained
        # Vec<Option<X>> (e.g. ExprDict.keys)
        if t.startswith("Vec<Option<"):
            return (f"{ind}for value in std::mem::take(&mut {access}.{field}) {{\n"
                    f"{ind}    if let Some(value) = value {{\n"
                    f"{ind}        self.{DISPATCH[inner]}(value);\n"
                    f"{ind}    }}\n"
                    f"{ind}}}")
        if t.startswith("Vec<"):
            return (f"{ind}for value in std::mem::take(&mut {access}.{field}) {{\n"
                    f"{ind}    self.{DISPATCH[inner]}(value);\n"
                    f"{ind}}}")
        # Option<Box<X>> (common nullable enum field)
        if t.startswith("Option<Box<"):
            return (f"{ind}if let Some(value) = {access}.{field} {{\n"
                    f"{ind}    self.{DISPATCH[inner]}(*value);\n"
                    f"{ind}}}")
        # plain Option<X> (nullable enum field, no Box)
        if t.startswith("Option<"):
            return (f"{ind}if let Some(value) = {access}.{field} {{\n"
                    f"{ind}    self.{DISPATCH[inner]}(value);\n"
                    f"{ind}}}")
        if t.startswith("Box<"):
            return f"{ind}self.{DISPATCH[inner]}(*{access}.{field});"
        # plain (non-wrapped) enum field, e.g. WithItem.context_expr
        return f"{ind}self.{DISPATCH[inner]}({access}.{field});"
    # NO_METHOD composite -> inline it here.
    return field_walk_inline(field, t, rec, access)


def field_walk_inline(field, typ, rec, access="node"):
    """Full walk statement for a NO_METHOD composite field (no visit_*)."""
    ind = indent_for(rec)
    t = typ.strip()
    contained = strip(t)
    if t.startswith("Vec<"):
        body = _inline_body("value", contained, rec + 1)
        return (f"{ind}for value in std::mem::take(&mut {access}.{field}) {{\n"
                + "\n".join("    " + b for b in body.splitlines())
                + f"\n{ind}}}")
    if t.startswith("Option<Box<"):
        body = _inline_body("value", contained, rec + 1)
        return (f"{ind}if let Some(value) = {access}.{field} {{\n"
                + "\n".join("    " + b for b in body.splitlines())
                + f"\n{ind}}}")
    if t.startswith("Box<"):
        body = _inline_body("inner", contained, rec + 1)
        return (f"{ind}let inner = *{access}.{field};\n"
                + "\n".join(body.splitlines()))
    # plain composite field
    body = _inline_body(f"{access}.{field}", contained, rec)
    return "\n".join(body.splitlines())


def _inline_body(access, inner_t, rec):
    """emit()+walks for a NO_METHOD composite reached via `access`."""
    ind = indent_for(rec)
    extra_expr, walks = collect(inner_t, access, rec)
    out = [f"{ind}self.emit({label(inner_t)}, {access}.range(), {extra_expr});"]
    if walks:
        out.append(f"{ind}self.indent += 1;")
        out.extend(walks)
        out.append(f"{ind}self.indent -= 1;")
    return "\n".join(out)


def collect(name, access, rec):
    """For struct `name` at `access`, return (extra_expr_str, [walk_lines])."""
    fields = STRUCTS[name]
    extras, walks = [], []
    for field, typ in fields:
        if field == "range":
            continue
        contained = strip(typ)
        if contained is not None:
            walks.append(field_walk(field, typ, rec, access))
        else:
            for fmt, args in leaf_renders(field, typ, access):
                extras.append((fmt, args))
    extra_expr = "None"
    if extras:
        parts = [f"            format_args!({fmt}, {args})," for fmt, args in extras]
        extra_expr = "Some(&[\n" + "\n".join(parts) + "\n        ])"
    return extra_expr, walks


def leaf_renders(field, typ, access):
    """Yield (format_literal, args_str) for a leaf field via `access`."""
    a = f"{access}.{field}"
    if typ == "Identifier":
        yield ('"{f} = {v:?}"', f'f = "{field}", v = {a}.as_str()')
    elif typ == "String":
        yield ('"{f} = {v:?}"', f'f = "{field}", v = &{a}')
    elif typ == "Constant":
        yield ('"{f} = {v}"', f'f = "{field}", v = ConstantDisplay(&{a})')
    elif typ == "Int":
        yield ('"{f} = {v}"', f'f = "{field}", v = {a}')
    elif typ == "bool":
        yield ('"{f} = {v}"', f'f = "{field}", v = {a}')
    elif typ in {"BoolOp", "Operator", "CmpOp", "UnaryOp", "ExprContext",
                 "ConversionFlag"}:
        yield ('"{f} = {v:?}"', f'f = "{field}", v = {a}')


HEADER = """// AUTO-GENERATED by gen_overrides.py -- do not edit by hand.
// Each override emits the node line and manually walks its children so that
// indentation (self.indent) and child nodes render correctly.
use rustpython_parser::ast::{self, *};
use rustpython_parser::ast::Visitor;
use rustpython_parser::text_size::TextRange;

/// Minimal Display for a `Constant` leaf.
struct ConstantDisplay<'a>(&'a Constant);
impl std::fmt::Display for ConstantDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Constant::None => write!(f, "None"),
            Constant::Ellipsis => write!(f, "Ellipsis"),
            Constant::Bool(b) => write!(f, "{b}"),
            Constant::Str(s) => write!(f, "{s:?}"),
            Constant::Bytes(b) => write!(f, "b{} bytes", b.len()),
            Constant::Int(i) => write!(f, "{i}"),
            Constant::Float(x) => write!(f, "{x}"),
            Constant::Complex { real, imag } => write!(f, "{real}+{imag}i"),
            Constant::Tuple(items) => write!(f, "({} items)", items.len()),
        }
    }
}
"""

lines = [HEADER, "", "impl Visitor<TextRange> for crate::DlpDumper {", ""]

for name in sorted(DISPATCH):
    if name not in STRUCTS:
        continue
    method = DISPATCH[name]
    extra_expr, walks = collect(name, "node", 0)
    body = [f"    fn {method}(&mut self, mut node: {name}<TextRange>) {{"]
    body.append(f"        self.emit({label(name)}, node.range(), {extra_expr});")
    if walks:
        body.append("        self.indent += 1;")
        body.extend(walks)
        body.append("        self.indent -= 1;")
    body.append("    }")
    lines.append("\n".join(body))
    lines.append("")

lines.append("}")

open("src/overrides.rs", "w").write("\n".join(lines))
print("wrote src/overrides.rs:", len(lines), "lines,",
      sum(1 for n in DISPATCH if n in STRUCTS), "overrides")

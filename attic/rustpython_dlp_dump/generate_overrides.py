#!/usr/bin/env python3
# Generates src/overrides.rs: a complete `impl Visitor for DlpDumper` that emits
# one .dlp line per AST node. Leaf fields are formatted by fmt_* helpers; the
# included generic_visit_* bodies (copied verbatim from rustpython-ast's
# generated visitor.rs) do the recursion.
import re, glob, os

PP = glob.glob("/home/adukhan/.cargo/registry/src/*/rustpython-ast-0.4.0")[0]
SRC = f"{PP}/src/gen/visitor.rs"

# Per-type leaf-field formatting. Keys are the rustpython-ast node type names.
HOWS = {
    "StmtImport": {"names": "fmt_aliases(&node.names)"},
    "StmtImportFrom": {"module": "fmt_opt_str(&node.module)", "names": "fmt_aliases(&node.names)", "level": "node.level"},
    "StmtGlobal": {"names": "fmt_idents(&node.names)"},
    "StmtNonlocal": {"names": "fmt_idents(&node.names)"},
    "StmtExpr": {"value": "fmt_expr(&node.value)"},
    "StmtReturn": {"value": "fmt_opt_expr(&node.value)"},
    "StmtDelete": {"targets": "fmt_exprs(&node.targets)"},
    "StmtAssign": {"targets": "fmt_exprs(&node.targets)", "value": "fmt_expr(&node.value)"},
    "StmtAugAssign": {"target": "fmt_expr(&node.target)", "op": "fmt_operator(&node.op)", "value": "fmt_expr(&node.value)"},
    "StmtAnnAssign": {"target": "fmt_expr(&node.target)", "annotation": "fmt_expr(&node.annotation)", "value": "fmt_opt_expr(&node.value)", "simple": "node.simple"},
    "StmtTypeAlias": {"name": "fmt_expr(&node.name)", "type_params": "fmt_type_params(&node.type_params)", "value": "fmt_expr(&node.value)"},
    "StmtFor": {"target": "fmt_expr(&node.target)", "iter": "fmt_expr(&node.iter)", "body": "fmt_stmts(&node.body)", "orelse": "fmt_stmts(&node.orelse)"},
    "StmtAsyncFor": {"target": "fmt_expr(&node.target)", "iter": "fmt_expr(&node.iter)", "body": "fmt_stmts(&node.body)", "orelse": "fmt_stmts(&node.orelse)"},
    "StmtWhile": {"test": "fmt_expr(&node.test)", "body": "fmt_stmts(&node.body)", "orelse": "fmt_stmts(&node.orelse)"},
    "StmtIf": {"test": "fmt_expr(&node.test)", "body": "fmt_stmts(&node.body)", "orelse": "fmt_stmts(&node.orelse)"},
    "StmtWith": {"items": "fmt_with_items(&node.items)", "body": "fmt_stmts(&node.body)"},
    "StmtAsyncWith": {"items": "fmt_with_items(&node.items)", "body": "fmt_stmts(&node.body)"},
    "StmtMatch": {"subject": "fmt_expr(&node.subject)", "cases": "fmt_match_cases(&node.cases)"},
    "StmtRaise": {"exc": "fmt_opt_expr(&node.exc)", "cause": "fmt_opt_expr(&node.cause)"},
    "StmtTry": {"body": "fmt_stmts(&node.body)", "handlers": "fmt_handlers(&node.handlers)", "orelse": "fmt_stmts(&node.orelse)", "finalbody": "fmt_stmts(&node.finalbody)"},
    "StmtTryStar": {"body": "fmt_stmts(&node.body)", "handlers": "fmt_handlers(&node.handlers)", "orelse": "fmt_stmts(&node.orelse)", "finalbody": "fmt_stmts(&node.finalbody)"},
    "StmtAssert": {"test": "fmt_expr(&node.test)", "msg": "fmt_opt_expr(&node.msg)"},
    "StmtBreak": {},
    "StmtContinue": {},
    "StmtPass": {},
    "Alias": {"name": "node.name.as_str()", "asname": "fmt_opt_str(&node.asname)"},
    "WithItem": {"context_expr": "fmt_expr(&node.context_expr)", "optional_vars": "fmt_opt_expr(&node.optional_vars)"},
    "MatchCase": {"pattern": "fmt_pattern(&node.pattern)", "guard": "fmt_opt_expr(&node.guard)", "body": "fmt_stmts(&node.body)"},
    "MatchValue": {"value": "fmt_expr(&node.value)"},
    "MatchSingleton": {"value": "fmt_singleton(&node.value)"},
    "MatchSequence": {"patterns": "fmt_patterns(&node.patterns)"},
    "MatchMapping": {"keys": "fmt_exprs(&node.keys)", "patterns": "fmt_patterns(&node.patterns)", "rest": "fmt_opt_str(&node.rest)"},
    "MatchClass": {"cls": "fmt_expr(&node.cls)", "patterns": "fmt_patterns(&node.patterns)", "kwd_attrs": "fmt_idents(&node.kwd_attrs)", "kwd_patterns": "fmt_patterns(&node.kwd_patterns)"},
    "MatchStar": {"name": "fmt_opt_str(&node.name)"},
    "MatchAs": {"name": "fmt_opt_str(&node.name)", "pattern": "fmt_opt_pattern(&node.pattern)"},
    "MatchOr": {"patterns": "fmt_patterns(&node.patterns)"},
    "ExceptHandlerExceptHandler": {"type": "fmt_opt_expr(&node.type_)", "name": "fmt_opt_str(&node.name)", "body": "fmt_stmts(&node.body)"},
    "Arg": {"arg": "node.arg.as_str()", "annotation": "fmt_opt_expr(&node.annotation)"},
    "Arguments": {"posonlyargs": "fmt_args(&node.posonlyargs)", "args": "fmt_args(&node.args)", "vararg": "fmt_opt_arg(&node.vararg)", "kwonlyargs": "fmt_args(&node.kwonlyargs)", "kw_defaults": "fmt_opt_exprs(&node.kw_defaults)", "kwarg": "fmt_opt_arg(&node.kwarg)", "defaults": "fmt_exprs(&node.defaults)"},
    "Keyword": {"arg": "fmt_opt_str(&node.arg)", "value": "fmt_expr(&node.value)"},
    "Comprehension": {"target": "fmt_expr(&node.target)", "iter": "fmt_expr(&node.iter)", "ifs": "fmt_exprs(&node.ifs)", "is_async": "node.is_async"},
    "ExprBoolOp": {"op": "fmt_boolop(&node.op)", "values": "fmt_exprs(&node.values)"},
    "ExprBinOp": {"left": "fmt_expr(&node.left)", "op": "fmt_operator(&node.op)", "right": "fmt_expr(&node.right)"},
    "ExprUnaryOp": {"op": "fmt_unaryop(&node.op)", "operand": "fmt_expr(&node.operand)"},
    "ExprLambda": {"args": "fmt_opt_arguments(&node.args)", "body": "fmt_expr(&node.body)"},
    "ExprIfExp": {"test": "fmt_expr(&node.test)", "body": "fmt_expr(&node.body)", "orelse": "fmt_expr(&node.orelse)"},
    "ExprDict": {"keys": "fmt_opt_exprs(&node.keys)", "values": "fmt_exprs(&node.values)"},
    "ExprSet": {"elts": "fmt_exprs(&node.elts)"},
    "ExprListComp": {"elt": "fmt_expr(&node.elt)", "generators": "fmt_comps(&node.generators)"},
    "ExprSetComp": {"elt": "fmt_expr(&node.elt)", "generators": "fmt_comps(&node.generators)"},
    "ExprDictComp": {"key": "fmt_expr(&node.key)", "value": "fmt_expr(&node.value)", "generators": "fmt_comps(&node.generators)"},
    "ExprGeneratorExp": {"elt": "fmt_expr(&node.elt)", "generators": "fmt_comps(&node.generators)"},
    "ExprAwait": {"value": "fmt_expr(&node.value)"},
    "ExprYield": {"value": "fmt_opt_expr(&node.value)"},
    "ExprYieldFrom": {"value": "fmt_expr(&node.value)"},
    "ExprCompare": {"left": "fmt_expr(&node.left)", "ops": "fmt_cmpops(&node.ops)", "comparators": "fmt_exprs(&node.comparators)"},
    "ExprCall": {"func": "fmt_expr(&node.func)", "args": "fmt_exprs(&node.args)", "keywords": "fmt_keywords(&node.keywords)"},
    "ExprFormattedValue": {"value": "fmt_expr(&node.value)", "conversion": "fmt_conversion(&node.conversion)", "format_spec": "fmt_opt_expr(&node.format_spec)"},
    "ExprJoinedStr": {"values": "fmt_exprs(&node.values)"},
    "ExprConstant": {"value": "fmt_constant(&node.value)"},
    "ExprAttribute": {"value": "fmt_expr(&node.value)", "attr": "node.attr.as_str(), ctx=expr_context(&node.ctx)"},
    "ExprSubscript": {"value": "fmt_expr(&node.value)", "slice": "fmt_expr(&node.slice), ctx=expr_context(&node.ctx)"},
    "ExprStarred": {"value": "fmt_expr(&node.value), ctx=expr_context(&node.ctx)"},
    "ExprName": {"id": "node.id.as_str(), ctx=expr_context(&node.ctx)"},
    "ExprList": {"elts": "fmt_exprs(&node.elts), ctx=expr_context(&node.ctx)"},
    "ExprTuple": {"elts": "fmt_exprs(&node.elts), ctx=expr_context(&node.ctx)"},
    "ExprSlice": {"lower": "fmt_opt_expr(&node.lower)", "upper": "fmt_opt_expr(&node.upper)", "step": "fmt_opt_expr(&node.step)"},
    "ExprNamedExpr": {"target": "fmt_expr(&node.target)", "value": "fmt_expr(&node.value)"},
    "Decorator": {"expression": "fmt_expr(&node.expression)"},
    "Pattern": {},  # placeholder not used
    "TypeParamTypeVar": {"name": "node.name.as_str()", "bound": "fmt_opt_expr(&node.bound)", "variance": "fmt_variance(&node.variance)"},
    "TypeParamParamSpec": {"name": "node.name.as_str()"},
    "TypeParamTypeVarTuple": {"name": "node.name.as_str()"},
    "StmtFunctionDef": {"name": "expr_literal(&*node.name)", "args": "fmt_arg(&node.args)", "decorator_list": "fmt_exprs(&node.decorator_list)", "returns": "fmt_opt_expr(&node.returns)", "type_params": "fmt_type_params(&node.type_params)"},
    "StmtAsyncFunctionDef": {"name": "expr_literal(&*node.name)", "args": "fmt_arg(&node.args)", "decorator_list": "fmt_exprs(&node.decorator_list)", "returns": "fmt_opt_expr(&node.returns)", "type_params": "fmt_type_params(&node.type_params)"},
    "StmtClassDef": {"name": "expr_literal(&*node.name)", "bases": "fmt_exprs(&node.bases)", "keywords": "fmt_keywords(&node.keywords)", "body": "fmt_stmts(&node.body)", "decorator_list": "fmt_exprs(&node.decorator_list)", "type_params": "fmt_type_params(&node.type_params)"},
}

# Field order for each node, as it appears in the rustpython-ast struct.
LEAF_FIRST = {
    "StmtImport": ["names"],
    "StmtImportFrom": ["module", "names", "level"],
    "StmtGlobal": ["names"],
    "StmtNonlocal": ["names"],
    "StmtExpr": ["value"],
    "StmtReturn": ["value"],
    "StmtDelete": ["targets"],
    "StmtAssign": ["targets", "value"],
    "StmtAugAssign": ["target", "op", "value"],
    "StmtAnnAssign": ["target", "annotation", "value", "simple"],
    "StmtTypeAlias": ["name", "type_params", "value"],
    "StmtFor": ["target", "iter", "body", "orelse"],
    "StmtAsyncFor": ["target", "iter", "body", "orelse"],
    "StmtWhile": ["test", "body", "orelse"],
    "StmtIf": ["test", "body", "orelse"],
    "StmtWith": ["items", "body"],
    "StmtAsyncWith": ["items", "body"],
    "StmtMatch": ["subject", "cases"],
    "StmtRaise": ["exc", "cause"],
    "StmtTry": ["body", "handlers", "orelse", "finalbody"],
    "StmtTryStar": ["body", "handlers", "orelse", "finalbody"],
    "StmtAssert": ["test", "msg"],
    "StmtBreak": [],
    "StmtContinue": [],
    "StmtPass": [],
    "Alias": ["name", "asname"],
    "WithItem": ["context_expr", "optional_vars"],
    "MatchCase": ["pattern", "guard", "body"],
    "MatchValue": ["value"],
    "MatchSingleton": ["value"],
    "MatchSequence": ["patterns"],
    "MatchMapping": ["keys", "patterns", "rest"],
    "MatchClass": ["cls", "patterns", "kwd_attrs", "kwd_patterns"],
    "MatchStar": ["name"],
    "MatchAs": ["name", "pattern"],
    "MatchOr": ["patterns"],
    "ExceptHandlerExceptHandler": ["type_", "name", "body"],
    "Arg": ["arg", "annotation"],
    "Arguments": ["posonlyargs", "args", "vararg", "kwonlyargs", "kw_defaults", "kwarg", "defaults"],
    "Keyword": ["arg", "value"],
    "Comprehension": ["target", "iter", "ifs", "is_async"],
    "ExprBoolOp": ["op", "values"],
    "ExprBinOp": ["left", "op", "right"],
    "ExprUnaryOp": ["op", "operand"],
    "ExprLambda": ["args", "body"],
    "ExprIfExp": ["test", "body", "orelse"],
    "ExprDict": ["keys", "values"],
    "ExprSet": ["elts"],
    "ExprListComp": ["elt", "generators"],
    "ExprSetComp": ["elt", "generators"],
    "ExprDictComp": ["key", "value", "generators"],
    "ExprGeneratorExp": ["elt", "generators"],
    "ExprAwait": ["value"],
    "ExprYield": ["value"],
    "ExprYieldFrom": ["value"],
    "ExprCompare": ["left", "ops", "comparators"],
    "ExprCall": ["func", "args", "keywords"],
    "ExprFormattedValue": ["value", "conversion", "format_spec"],
    "ExprJoinedStr": ["values"],
    "ExprConstant": ["value"],
    "ExprAttribute": ["value", "attr", "ctx"],
    "ExprSubscript": ["value", "slice", "ctx"],
    "ExprStarred": ["value", "ctx"],
    "ExprName": ["id", "ctx"],
    "ExprList": ["elts", "ctx"],
    "ExprTuple": ["elts", "ctx"],
    "ExprSlice": ["lower", "upper", "step"],
    "ExprNamedExpr": ["target", "value"],
    "Decorator": ["expression"],
    "TypeParamTypeVar": ["name", "bound", "variance"],
    "TypeParamParamSpec": ["name"],
    "TypeParamTypeVarTuple": ["name"],
    "StmtFunctionDef": ["name", "args", "decorator_list", "returns", "type_params"],
    "StmtAsyncFunctionDef": ["name", "args", "decorator_list", "returns", "type_params"],
    "StmtClassDef": ["name", "bases", "keywords", "body", "decorator_list", "type_params"],
}

# Parse the generated visitor.rs to learn every typed visit_* signature.
src = open(SRC).read()
sig_re = re.compile(r"^    fn (\w+)\(&mut self, node: ([\w]+)<R>\) \{", re.M)
fns = [(n, t) for n, t in sig_re.findall(src) if n.startswith("visit_")]

# Extract the trait's default-method bodies. Keep only the per-type
# generic_visit_* recursion helpers plus the visit_stmt/visit_expr dispatchers;
# the typed visit_* defaults are replaced by our emit overrides below.
_trait_body = re.search(r"pub trait Visitor.*?\{\n(.*)\n\}", src, re.S).group(1)
def _method_blocks(text):
    blocks = []
    depth = 0
    cur = ""
    for line in text.splitlines(keepends=True):
        if depth == 0 and line.strip().startswith("fn "):
            cur = line
        else:
            cur += line
        depth += line.count("{") - line.count("}")
        if depth == 0 and cur.strip():
            blocks.append(cur)
            cur = ""
    return blocks
_kept = []
for b in _method_blocks(_trait_body):
    name = re.search(r"fn (\w+)", b).group(1)
    if name in ("visit_stmt", "visit_expr") or name.startswith("generic_visit_"):
        _kept.append(b.rstrip())
trait_body = "\n".join(_kept)

lines = []
lines.append("// GENERATED by generate_overrides.py — do not edit by hand.")
lines.append("impl<R> ast::Visitor<R> for DlpDumper {")
lines.append(trait_body.rstrip())
lines.append("")

for n, t in fns:
    if n in ("visit_stmt", "visit_expr"):
        continue  # keep trait defaults (dispatchers)
    label = t
    if n.startswith("visit_stmt_"):
        pass
    leaf = LEAF_FIRST.get(t, None)
    if leaf is None:
        # Not a node type we emit (e.g. visit_operator, visit_expr_context...).
        continue
    parts = []
    for f in leaf:
        if f in HOWS[t]:
            parts.append(f"{f}={{{HOWS[t][f]}}}")
    args = ", ".join(parts)
    lines.append(f"    fn {n}(&mut self, node: {t}<R>) {{")
    lines.append(f"        self.emit(\"{label}\", self.locate(node.range()), format_args!(\"{args}\"));")
    lines.append(f"        self.generic_visit_{n[len('visit_'):]}(node);")
    lines.append("    }")
    lines.append("")

lines.append("}")
open("src/overrides.rs", "w").write("\n".join(lines) + "\n")
print("wrote src/overrides.rs with", sum(1 for l in lines if l.startswith("    fn visit_") and not l.startswith("    fn visit_stmt") and not l.startswith("    fn visit_expr")), "visit_ methods")

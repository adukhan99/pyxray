exec(open("generate_overrides.py").read().split("lines = []")[0])
print("fns:", len(fns))
gen=0; skipno=0; skipdisp=0
for n,t in fns:
    if n in ("visit_stmt","visit_expr"):
        skipdisp+=1; continue
    if LEAF_FIRST.get(t) is None:
        skipno+=1; continue
    gen+=1
print("generated:",gen,"skip_no_leaf:",skipno,"skip_dispatcher:",skipdisp)

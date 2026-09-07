exec(open("generate_overrides.py").read().split("lines = []")[0])
print("== fns types without LEAF_FIRST (need dispatcher or noop) ==")
for n,t in fns:
    if t not in LEAF_FIRST:
        print("  %s  ->  %s" % (n, t))

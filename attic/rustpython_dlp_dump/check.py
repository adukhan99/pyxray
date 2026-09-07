import json, re
d=json.load(open('ast_data.json'))
def snake(n):
    s=re.sub(r'(.)([A-Z][a-z]+)', r'\1_\2', n)
    s=re.sub(r'([a-z0-9])([A-Z])', r'\1_\2', s)
    return s.lower()
needed=sorted("generic_visit_"+snake(n) for n in d['structs'])
gv=open('/tmp/gv.txt').read().split()
print("needed:",len(needed),"have_in_trait:",len(gv))
print("missing_in_trait:", sorted(set(needed)-set(gv)))
print("extra_in_trait:", sorted(set(gv)-set(needed)))
